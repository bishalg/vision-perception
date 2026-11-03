#!/usr/bin/env node
/**
 * Minimal in-memory S3 stand-in for the live record/share e2e (`run-record-live.sh`), dev tooling only.
 *
 * `asd-backend serve` and `asd-backend worker` are separate processes, so `ASD_BLOB_BACKEND=memory` cannot hand the
 * worker's case-package ZIP to the API. With no Docker (MinIO) on the machine, this gives both processes one shared
 * store over the S3 REST calls `blob/s3.rs` makes: HeadBucket, PutObject, GetObject, DeleteObject (path style, no
 * signature check, no multipart: every artifact in the e2e is far below the 32 MiB multipart threshold).
 *
 * Usage: node fake-s3.mjs <port>
 */
import http from "node:http";

const port = Number(process.argv[2] ?? process.env.FAKE_S3_PORT ?? 0);
const objects = new Map();

/** `aws-chunked` bodies (`<hex>[;chunk-signature=..]\r\n<data>\r\n ... 0\r\n<trailers>\r\n`) back to plain bytes. */
function dechunk(buf) {
  const out = [];
  let i = 0;
  while (i < buf.length) {
    const eol = buf.indexOf("\r\n", i);
    if (eol < 0) break;
    const size = parseInt(buf.subarray(i, eol).toString("latin1").split(";")[0], 16);
    if (!Number.isFinite(size) || size === 0) break;
    out.push(buf.subarray(eol + 2, eol + 2 + size));
    i = eol + 2 + size + 2;
  }
  return Buffer.concat(out);
}

function xmlError(res, status, code) {
  res.writeHead(status, { "Content-Type": "application/xml" });
  res.end(`<?xml version="1.0" encoding="UTF-8"?><Error><Code>${code}</Code><Message>${code}</Message></Error>`);
}

const server = http.createServer((req, res) => {
  const url = new URL(req.url ?? "/", "http://s3.local");
  const [, bucket, ...rest] = url.pathname.split("/");
  const key = decodeURIComponent(rest.join("/"));
  const chunks = [];
  req.on("data", (c) => chunks.push(c));
  req.on("end", () => {
    if (!bucket) return xmlError(res, 400, "InvalidRequest");
    if (!key) {
      // HeadBucket / CreateBucket: the bucket always exists.
      res.writeHead(200);
      return res.end();
    }
    const id = `${bucket}/${key}`;
    switch (req.method) {
      case "PUT": {
        let body = Buffer.concat(chunks);
        const sha = String(req.headers["x-amz-content-sha256"] ?? "");
        if (String(req.headers["content-encoding"] ?? "").includes("aws-chunked") || sha.startsWith("STREAMING")) {
          body = dechunk(body);
        }
        objects.set(id, { body, type: String(req.headers["content-type"] ?? "application/octet-stream") });
        res.writeHead(200, { ETag: `"${objects.size}"` });
        return res.end();
      }
      case "GET":
      case "HEAD": {
        const o = objects.get(id);
        if (!o) return xmlError(res, 404, "NoSuchKey");
        res.writeHead(200, { "Content-Type": o.type, "Content-Length": String(o.body.length), ETag: '"1"' });
        return res.end(req.method === "GET" ? o.body : undefined);
      }
      case "DELETE":
        objects.delete(id);
        res.writeHead(204);
        return res.end();
      default:
        return xmlError(res, 501, "NotImplemented");
    }
  });
});

server.listen(port, "127.0.0.1", () => {
  console.log(`fake-s3 listening on ${server.address().port}`);
});
