import Foundation
import XCTest

/// Reads the golden vectors shared with `rust/crates/asd-core/tests/golden_*.rs` (FFI-4) and
/// compares JSON documents the way those tests do: floats to 1e-9, strings, booleans and key
/// sets exactly.
enum Golden {
    static let tolerance = 1e-9

    /// `rust/crates/asd-core/tests/golden`, found from this file's location in the repo.
    static let root: URL = {
        var url = URL(fileURLWithPath: #filePath)
        // .../domains/asd_screening/ios/Tests/CurvAsdGazeKitTests/Support/GoldenSupport.swift
        for _ in 0..<7 { url.deleteLastPathComponent() }
        return url.appendingPathComponent("rust/crates/asd-core/tests/golden", isDirectory: true)
    }()

    struct Vector {
        let name: String
        let input: Any
        let expected: Any
    }

    static func files(in family: String) throws -> [URL] {
        let dir = root.appendingPathComponent(family, isDirectory: true)
        return try FileManager.default.contentsOfDirectory(at: dir, includingPropertiesForKeys: nil)
            .filter { $0.pathExtension == "json" }
            .sorted { $0.lastPathComponent < $1.lastPathComponent }
    }

    static func object(at url: URL) throws -> [String: Any] {
        let data = try Data(contentsOf: url)
        return try XCTUnwrap(JSONSerialization.jsonObject(with: data, options: [.fragmentsAllowed]) as? [String: Any], url.path)
    }

    /// Vectors of a file in either layout: `asd-golden/1` (`vectors: [...]`) or one case per
    /// file (`case`, `input`, `expected`). Returns the file's `function` too.
    static func vectors(at url: URL) throws -> (function: String, vectors: [Vector]) {
        let doc = try object(at: url)
        let function = try XCTUnwrap(doc["function"] as? String, "\(url.lastPathComponent): function")
        if let list = doc["vectors"] as? [[String: Any]] {
            XCTAssertEqual(doc["schema"] as? String, "asd-golden/1", url.lastPathComponent)
            let vectors = try list.map { v in
                Vector(
                    name: "\(url.lastPathComponent)#\(try XCTUnwrap(v["name"] as? String))",
                    input: try XCTUnwrap(v["input"]),
                    expected: try XCTUnwrap(v["expected"])
                )
            }
            return (function, vectors)
        }
        let name = (doc["case"] as? String) ?? url.deletingPathExtension().lastPathComponent
        return (function, [Vector(
            name: "\(url.lastPathComponent)#\(name)",
            input: try XCTUnwrap(doc["input"]),
            expected: try XCTUnwrap(doc["expected"])
        )])
    }

    /// Decodes a JSON fragment into a Swift type.
    static func decode<T: Decodable>(_ type: T.Type, _ value: Any) throws -> T {
        let data = try JSONSerialization.data(withJSONObject: value, options: [.fragmentsAllowed])
        return try JSONDecoder().decode(type, from: data)
    }

    /// The JSON form of a Swift value (what `JSONEncoder` writes).
    static func json<T: Encodable>(_ value: T) throws -> Any {
        try JSONSerialization.jsonObject(with: JSONEncoder().encode(value), options: [.fragmentsAllowed])
    }

    static func assertClose(_ actual: Any, _ expected: Any, _ path: String, file: StaticString = #filePath, line: UInt = #line) {
        switch (actual, expected) {
        case let (a as NSNumber, e as NSNumber):
            let aBool = CFGetTypeID(a) == CFBooleanGetTypeID()
            let eBool = CFGetTypeID(e) == CFBooleanGetTypeID()
            if aBool || eBool {
                XCTAssertTrue(aBool && eBool && a.boolValue == e.boolValue, "\(path): \(a) != \(e)", file: file, line: line)
            } else {
                XCTAssertEqual(a.doubleValue, e.doubleValue, accuracy: tolerance, "\(path)", file: file, line: line)
            }
        case let (a as [String: Any], e as [String: Any]):
            XCTAssertEqual(Set(a.keys), Set(e.keys), "\(path): key sets differ", file: file, line: line)
            for (key, ev) in e {
                guard let av = a[key] else { continue }
                assertClose(av, ev, "\(path).\(key)", file: file, line: line)
            }
        case let (a as [Any], e as [Any]):
            XCTAssertEqual(a.count, e.count, "\(path): array lengths differ", file: file, line: line)
            for (i, (av, ev)) in zip(a, e).enumerated() {
                assertClose(av, ev, "\(path)[\(i)]", file: file, line: line)
            }
        case let (a as String, e as String):
            XCTAssertEqual(a, e, path, file: file, line: line)
        case (is NSNull, is NSNull):
            break
        default:
            XCTFail("\(path): \(actual) != \(expected)", file: file, line: line)
        }
    }
}
