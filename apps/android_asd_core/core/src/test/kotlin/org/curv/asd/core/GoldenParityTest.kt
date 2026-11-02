// owner: WP-A.12
package org.curv.asd.core

import java.io.File
import kotlin.math.abs
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.booleanOrNull
import kotlinx.serialization.json.double
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.long
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Test

/**
 * FFI-4 for Android (AND-2): the Kotlin bindings, calling the host `libasd_ffi` through JNA,
 * pass the same golden vectors as `cargo test -p asd-core`
 * (`rust/crates/asd-core/tests/golden/{scoring,flag,metrics,aggregate,quality}`) and as the
 * Swift kit's `GoldenParityTests`, which this test mirrors. Documents are compared the way
 * those tests do: numbers to 1e-9, strings, booleans and key sets exactly.
 *
 * The session contract golden (`expected-session-v1.8.json`) is compared byte for byte: the
 * request body Rust builds for it must equal the committed RFC 8785 (JCS) form
 * (`src/test/resources/golden/expected-session-v1.8.jcs.json`).
 *
 * Every golden file must be consumed here or be listed as having no FFI entry point
 * ([everyGoldenFileIsCovered]), so a new vector file cannot be skipped silently.
 *
 * Run with `scripts/build_asd_android.sh --test`, which builds the host library and sets
 * `jna.library.path` and `asd.repoRoot`.
 */
class GoldenParityTest {

    private val repoRoot = File(
        System.getProperty("asd.repoRoot")
            ?: error("asd.repoRoot is not set; run scripts/build_asd_android.sh --test"),
    )
    private val goldenRoot = File(repoRoot, "rust/crates/asd-core/tests/golden")

    private data class Vector(val name: String, val input: JsonElement, val expected: JsonElement)

    // MARK: - Golden reader (the layouts of GoldenSupport.swift)

    private fun files(family: String): List<File> =
        (File(goldenRoot, family).listFiles { f -> f.extension == "json" } ?: emptyArray())
            .sortedBy { it.name }

    /** `asd-golden/1` (`vectors: [...]`) or one case per file (`case`, `input`, `expected`). */
    private fun vectorsAt(file: File): Pair<String, List<Vector>> {
        val doc = Json.parseToJsonElement(file.readText()).jsonObject
        val function = doc.getValue("function").jsonPrimitive.content
        val list = doc["vectors"]
        if (list is JsonArray) {
            assertEquals(file.name, "asd-golden/1", doc["schema"]?.jsonPrimitive?.content)
            return function to list.map { v ->
                val o = v.jsonObject
                Vector(
                    "${file.name}#${o.getValue("name").jsonPrimitive.content}",
                    o.getValue("input"),
                    o.getValue("expected"),
                )
            }
        }
        val name = doc["case"]?.jsonPrimitive?.content ?: file.nameWithoutExtension
        return function to listOf(
            Vector("${file.name}#$name", doc.getValue("input"), doc.getValue("expected")),
        )
    }

    private fun vectors(family: String, function: String): List<Vector> {
        val all = files(family).flatMap { f ->
            val (fn, vs) = vectorsAt(f)
            if (fn == function) vs else emptyList()
        }
        assertFalse("no golden vectors for $family/$function", all.isEmpty())
        return all
    }

    private fun JsonElement.field(key: String): JsonElement = jsonObject.getValue(key)

    private fun parse(json: String): JsonElement = Json.parseToJsonElement(json)

    private fun assertClose(actual: JsonElement, expected: JsonElement, path: String) {
        when {
            expected is JsonNull || actual is JsonNull ->
                assertTrue("$path: $actual != $expected", actual is JsonNull && expected is JsonNull)
            expected is JsonObject && actual is JsonObject -> {
                assertEquals("$path: key sets differ", expected.keys, actual.keys)
                for ((key, ev) in expected) assertClose(actual.getValue(key), ev, "$path.$key")
            }
            expected is JsonArray && actual is JsonArray -> {
                assertEquals("$path: array lengths differ", expected.size, actual.size)
                expected.zip(actual).forEachIndexed { i, (ev, av) -> assertClose(av, ev, "$path[$i]") }
            }
            expected is JsonPrimitive && actual is JsonPrimitive -> {
                val eBool = !expected.isString && expected.booleanOrNull != null
                val aBool = !actual.isString && actual.booleanOrNull != null
                when {
                    expected.isString || actual.isString || eBool || aBool ->
                        assertEquals(path, expected, actual)
                    else -> assertTrue(
                        "$path: expected ${expected.content}, got ${actual.content}",
                        abs(expected.double - actual.double) <= TOLERANCE,
                    )
                }
            }
            else -> fail("$path: $actual != $expected")
        }
    }

    // MARK: - Library and version

    @Test
    fun goldenTreeIsFound() {
        assertTrue(goldenRoot.path, goldenRoot.isDirectory)
    }

    /** `coreVersion()` is asd-core's version: the Rust workspace version it inherits. */
    @Test
    fun coreVersionMatchesAsdCore() {
        val core = File(repoRoot, "rust/crates/asd-core/Cargo.toml").readText()
        val expected = if (Regex("""(?m)^version\.workspace\s*=\s*true""").containsMatchIn(core)) {
            val workspace = File(repoRoot, "rust/Cargo.toml").readText()
            Regex("""(?ms)^\[workspace\.package\].*?^version\s*=\s*"([^"]+)"""")
                .find(workspace)!!.groupValues[1]
        } else {
            Regex("""(?m)^version\s*=\s*"([^"]+)"""").find(core)!!.groupValues[1]
        }
        assertEquals(expected, coreVersion())
    }

    // MARK: - Coverage

    @Test
    fun everyGoldenFileIsCovered() {
        for (family in COVERED.keys + NO_FFI_ENTRY.keys) {
            for (file in files(family)) {
                val (function, vs) = vectorsAt(file)
                val known = (COVERED[family].orEmpty() + NO_FFI_ENTRY[family].orEmpty())
                assertTrue("$family/${file.name}: function $function has no Kotlin parity test", function in known)
                assertFalse(file.name, vs.isEmpty())
            }
        }
    }

    // MARK: - Scoring (CORE-1..3)

    @Test
    fun scoreSessionMatchesGolden() {
        for (v in vectors("scoring", "score_session")) {
            val actual = scoreSessionJson(
                v.input.field("metricSets").toString(),
                v.input.field("baseline").toString(),
                v.input.field("ageMonths").jsonPrimitive.long,
                v.input.field("frames").toString(),
                v.input.field("scoredAtUtc").jsonPrimitive.content,
            )
            assertClose(parse(actual), v.expected, v.name)
        }
    }

    @Test
    fun formatScoredAtMatchesGolden() {
        for (v in vectors("scoring", "format_scored_at")) {
            val seconds = v.input.field("unixSeconds").jsonPrimitive.double
            val actual = JsonObject(mapOf("scoredAtUtc" to JsonPrimitive(formatScoredAt(seconds))))
            assertClose(actual, v.expected, v.name)
        }
    }

    @Test
    fun validateBaselinesMatchesGolden() {
        for (v in vectors("scoring", "validate_baselines")) {
            val problems = validateBaselinesJson(
                v.input.field("baseline").toString(),
                v.input.field("requiredVideoIds").jsonArray.map { it.jsonPrimitive.content },
            )
            val actual = JsonObject(mapOf("problems" to JsonArray(problems.map(::JsonPrimitive))))
            assertClose(actual, v.expected, v.name)
        }
    }

    // MARK: - Flag (CORE-4)

    /** The vectors use the default thresholds: the bundled default scoring profile. */
    @Test
    fun attentionFlagMatchesGolden() {
        val profile = defaultScoringProfileJson()
        for (v in vectors("flag", "assess")) {
            assertFalse("${v.name}: custom thresholds are not wired here", v.input.jsonObject.containsKey("thresholds"))
            val actual = assessFlagJson(v.input.field("score").toString(), profile)
            assertClose(parse(actual), v.expected, v.name)
        }
    }

    // MARK: - Quality gate (CORE-7)

    @Test
    fun qualityGateMatchesGolden() {
        for (v in vectors("quality", "evaluate_sample")) {
            val actual = evaluateQualityJson(
                v.input.field("sample").toString(),
                v.input.field("gate").toString(),
            )
            assertClose(parse(actual), v.expected, v.name)
        }
    }

    // MARK: - Metrics and aggregate (CORE-5, CORE-6)

    @Test
    fun metricExtractionMatchesGolden() {
        for (v in vectors("metrics", "extract")) {
            val actual = extractMetricsJson(v.input.field("frames").toString())
            assertClose(parse(actual), v.expected, v.name)
        }
    }

    @Test
    fun sessionAggregateMatchesGolden() {
        for (v in vectors("aggregate", "session")) {
            val actual = aggregateSessionJson(
                v.input.field("validFrames").toString(),
                v.input.field("durationSec").jsonPrimitive.double,
            )
            assertClose(parse(actual), v.expected, v.name)
        }
    }

    // MARK: - Session contract 1.8 (byte for byte after JCS)

    @Test
    fun sessionV18RequestBodyIsTheCanonicalGolden() {
        val fixture = File(repoRoot, "apps/ios_asd_gaze_app/Tests/Fixtures/expected-session-v1.8.json")
        val golden = javaClass.getResourceAsStream("/golden/expected-session-v1.8.jcs.json")!!
            .use { it.readBytes() }
        val request = syncBuildSessionRequestJson(
            fixture.readText(),
            "device-token",
            AsdClientInfo("EyeGaze-Android", "1.0.0", "1", "Android", "14"),
        )
        assertEquals("POST", request.method)
        assertArrayEquals(
            "body differs from src/test/resources/golden/expected-session-v1.8.jcs.json:\n" +
                request.body.decodeToString(),
            golden,
            request.body,
        )
        val sessionId = parse(fixture.readText()).field("sessionId").jsonPrimitive.content
        val idempotency = request.headers.single { it.name.equals("X-Idempotency-Key", ignoreCase = true) }
        assertEquals(sessionId, idempotency.value)
    }

    // MARK: - Sync schedule and errors

    @Test
    fun syncBackoffScheduleIsTheRustSchedule() {
        val schedule = (1u..5u).map { syncBackoffSchedule(it).toLong() }
        assertEquals(listOf(1L, 2L, 4L, 8L, 60L), schedule)
        assertEquals(6u, syncMaxAttempts())
    }

    @Test
    fun malformedJsonIsAnInvalidJsonError() {
        try {
            extractMetricsJson("not json")
            fail("expected AsdException.InvalidJson")
        } catch (e: AsdException.InvalidJson) {
            assertTrue(e.detail.isNotEmpty())
        }
    }

    private companion object {
        const val TOLERANCE = 1e-9

        /** Functions with an FFI entry point, by family (as `GoldenParityTests.covered`). */
        val COVERED = mapOf(
            "scoring" to setOf("score_session", "format_scored_at", "validate_baselines"),
            "flag" to setOf("assess"),
            "quality" to setOf("evaluate_sample"),
            "metrics" to setOf("extract"),
            "aggregate" to setOf("session"),
        )

        /**
         * Internal steps with no FFI entry point of their own; cargo checks them, and every
         * `score_session` vector runs them through the FFI.
         */
        val NO_FFI_ENTRY = mapOf(
            "scoring" to setOf("evaluate", "piecewise", "brackets", "score_video"),
            "metrics" to setOf("extract_clip"),
        )
    }
}
