"use server";

import { randomBytes } from "node:crypto";
import { revalidatePath } from "next/cache";
import type { ActionFailure, ActionResult } from "@/components/exports/types";
import { ApiError } from "@/lib/api";
import {
  DELIVERY_EVENTS,
  deliveryTriggerSchema,
  exportFormatSchema,
  exportModeSchema,
  integrationEnvironmentSchema,
  integrationKindSchema,
  integrationStatusSchema,
  type IntegrationConfig,
  type IntegrationKind,
  type IntegrationTestResult,
  type SshPublicKey,
} from "@/lib/api/exports";
import { authedApi } from "@/lib/auth/server";
import { pathsFor } from "@/lib/paths";

function failure(err: unknown): ActionFailure {
  if (err instanceof ApiError) {
    const params = err.problem?.invalid_params?.map((p) => `${p.name}: ${p.reason}`).join("; ");
    return { ok: false, slug: err.slug, status: err.status, ...(params ? { params } : {}) };
  }
  return { ok: false, slug: null, status: 0 };
}

const text = (fd: FormData, k: string) => String(fd.get(k) ?? "").trim();

function touch(orgId: string, id?: string): void {
  const ap = pathsFor(orgId).admin;
  revalidatePath(ap.integrations);
  if (id) revalidatePath(ap.integration(id));
}

/**
 * The config replaces the whole stored config, so every field the form shows is sent: only non-empty values, per
 * kind. The backend refuses unknown keys and validates the rest; nothing is checked here beyond the enum spellings.
 */
function configFrom(fd: FormData, kind: IntegrationKind): IntegrationConfig {
  const c: IntegrationConfig = {};
  const trigger = deliveryTriggerSchema.safeParse(text(fd, "trigger"));
  if (trigger.success) c.trigger = trigger.data;
  const format = exportFormatSchema.safeParse(text(fd, "format"));
  if (format.success) c.format = format.data;
  const mode = exportModeSchema.safeParse(text(fd, "mode"));
  if (mode.success) c.mode = mode.data;
  if (kind === "webhook") {
    if (text(fd, "url")) c.url = text(fd, "url");
    const events = fd.getAll("events").map(String).filter((e): e is (typeof DELIVERY_EVENTS)[number] =>
      (DELIVERY_EVENTS as readonly string[]).includes(e),
    );
    if (events.length) c.events = events;
  }
  if (kind === "sftp") {
    for (const k of ["host", "username", "remote_dir", "host_key_sha256", "file_prefix"] as const) {
      if (text(fd, k)) c[k] = text(fd, k);
    }
    const port = text(fd, "port");
    // A non-integer is sent as an out-of-range port so the backend rejects it with a field reason.
    if (port) c.port = Number.isInteger(Number(port)) ? Number(port) : -1;
  }
  return c;
}

export async function createIntegrationAction(orgId: string, formData: FormData): Promise<ActionResult<{ id: string }>> {
  const kind = integrationKindSchema.safeParse(text(formData, "kind"));
  if (!kind.success) return { ok: false, slug: "validation-failed", status: 400, params: "kind: unknown" };
  const environment = integrationEnvironmentSchema.safeParse(text(formData, "environment"));
  try {
    const created = await (await authedApi()).createIntegration(orgId, {
      kind: kind.data,
      name: text(formData, "name"),
      config: configFrom(formData, kind.data),
      ...(environment.success ? { environment: environment.data } : {}),
    });
    touch(orgId);
    return { ok: true, id: created.id };
  } catch (err) {
    return failure(err);
  }
}

export async function updateIntegrationAction(
  orgId: string,
  id: string,
  kind: IntegrationKind,
  formData: FormData,
): Promise<ActionResult> {
  const environment = integrationEnvironmentSchema.safeParse(text(formData, "environment"));
  const status = integrationStatusSchema.safeParse(text(formData, "status"));
  try {
    await (await authedApi()).updateIntegration(id, {
      name: text(formData, "name"),
      config: configFrom(formData, kind),
      ...(environment.success ? { environment: environment.data } : {}),
      ...(status.success ? { status: status.data } : {}),
    });
    touch(orgId, id);
    return { ok: true };
  } catch (err) {
    return failure(err);
  }
}

/**
 * PUT /v1/integrations/{id}/secret. With an empty field a fresh `whsec_` secret is generated here and returned once
 * for the user to copy; nothing is stored or logged by the console. A pasted secret is never echoed back.
 */
export async function putSecretAction(
  orgId: string,
  id: string,
  formData: FormData,
): Promise<ActionResult<{ generated?: string }>> {
  const given = text(formData, "secret");
  const secret = given || `whsec_${randomBytes(32).toString("base64")}`;
  try {
    await (await authedApi()).putIntegrationSecret(id, secret);
    touch(orgId, id);
    return given ? { ok: true } : { ok: true, generated: secret };
  } catch (err) {
    return failure(err);
  }
}

/** POST /v1/integrations/{id}/ssh-key: generate or rotate; only the public half comes back. */
export async function generateSshKeyAction(orgId: string, id: string): Promise<ActionResult<{ key: SshPublicKey }>> {
  try {
    const key = await (await authedApi()).generateIntegrationSshKey(id);
    touch(orgId, id);
    return { ok: true, key };
  } catch (err) {
    return failure(err);
  }
}

/** POST /v1/integrations/{id}/test: the backend sends a synthetic record and stores the outcome. */
export async function testIntegrationAction(
  orgId: string,
  id: string,
): Promise<ActionResult<{ result: IntegrationTestResult }>> {
  try {
    const result = await (await authedApi()).testIntegration(id);
    touch(orgId, id);
    return { ok: true, result };
  } catch (err) {
    return failure(err);
  }
}

/** DELETE /v1/integrations/{id}: archives the destination; 409 while deliveries to it are pending or sending. */
export async function deleteIntegrationAction(orgId: string, id: string): Promise<ActionResult> {
  try {
    await (await authedApi()).deleteIntegration(id);
    touch(orgId);
    return { ok: true };
  } catch (err) {
    return failure(err);
  }
}
