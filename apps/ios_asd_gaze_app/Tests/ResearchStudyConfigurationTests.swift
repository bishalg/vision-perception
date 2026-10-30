@testable import AsdGazeCollector
import XCTest

/// CNS-9 / ADR-0002 decision 5: the research module exists only when the
/// Info.plist carries a complete `ASDResearchStudy` dictionary with an https
/// ingest URL. Anything else fails closed (`nil`).
final class ResearchStudyConfigurationTests: XCTestCase {
    private static let validDictionary: [String: Any] = [
        "ingestURL": "https://ingest.example.org/api/v1/research/ingest",
        "studyIdentifier": "EXAMPLE-STUDY-1",
        "irbProtocolId": "ETHICS-REF-42",
        "institution": "Example University",
        "principalInvestigator": "Example Investigator",
        "consentVersion": "2027.1"
    ]

    private static func info(_ study: [String: Any]?) -> [String: Any] {
        var info: [String: Any] = ["CFBundleIdentifier": "org.example.test"]
        if let study { info[ResearchStudyConfiguration.infoPlistKey] = study }
        return info
    }

    private static func study(replacing key: String, with value: Any?) -> [String: Any] {
        var dict = validDictionary
        dict[key] = value
        return dict
    }

    // MARK: - Absent configuration

    func testInfoPlistKeyName() {
        XCTAssertEqual(ResearchStudyConfiguration.infoPlistKey, "ASDResearchStudy")
    }

    func testLoadIsNilWithoutResearchStudyKey() {
        XCTAssertNil(ResearchStudyConfiguration.load(from: Self.info(nil)))
        XCTAssertNil(ResearchStudyConfiguration.load(from: nil))
    }

    func testLoadIsNilWhenKeyIsNotADictionary() {
        let info: [String: Any] = [ResearchStudyConfiguration.infoPlistKey: "https://ingest.example.org"]
        XCTAssertNil(ResearchStudyConfiguration.load(from: info))
    }

    /// The default build (the test host) ships without a study (H7).
    func testDefaultBuildHasNoResearchStudy() {
        XCTAssertNil(Bundle.main.object(forInfoDictionaryKey: ResearchStudyConfiguration.infoPlistKey))
        XCTAssertNil(ResearchStudyConfiguration.load(bundle: .main))
        XCTAssertNil(ResearchStudyConfiguration.current)
        XCTAssertFalse(ResearchStudyConfiguration.isResearchEnabled)
    }

    // MARK: - Valid configuration

    func testValidConfigurationIsParsed() throws {
        let config = try XCTUnwrap(ResearchStudyConfiguration.load(from: Self.info(Self.validDictionary)))
        XCTAssertEqual(config.ingestURL, URL(string: "https://ingest.example.org/api/v1/research/ingest"))
        XCTAssertEqual(config.studyIdentifier, "EXAMPLE-STUDY-1")
        XCTAssertEqual(config.irbProtocolId, "ETHICS-REF-42")
        XCTAssertEqual(config.institution, "Example University")
        XCTAssertEqual(config.principalInvestigator, "Example Investigator")
        XCTAssertEqual(config.consentVersion, "2027.1")
    }

    func testFieldsAreTrimmed() throws {
        let dict = Self.study(replacing: "institution", with: "  Example University \n")
        let config = try ResearchStudyConfiguration(dictionary: dict)
        XCTAssertEqual(config.institution, "Example University")
    }

    func testHttpsWithPortIsAccepted() throws {
        let dict = Self.study(replacing: "ingestURL", with: "https://ingest.example.org:8443/ingest")
        XCTAssertNotNil(ResearchStudyConfiguration.load(from: Self.info(dict)))
    }

    func testUppercaseHttpsSchemeIsAccepted() throws {
        let dict = Self.study(replacing: "ingestURL", with: "HTTPS://ingest.example.org/ingest")
        XCTAssertNotNil(ResearchStudyConfiguration.load(from: Self.info(dict)))
    }

    // MARK: - https only

    func testHttpEndpointIsRejected() {
        let dict = Self.study(replacing: "ingestURL", with: "http://ingest.example.org/api/v1/research/ingest")
        XCTAssertNil(ResearchStudyConfiguration.load(from: Self.info(dict)))
        XCTAssertThrowsError(try ResearchStudyConfiguration(dictionary: dict)) { error in
            XCTAssertEqual(error as? ResearchStudyConfiguration.ValidationError, .insecureScheme("http"))
        }
    }

    /// The pre-WP-2.3 default endpoint. IP literals bypass ATS (trap 10).
    func testLegacyLoopbackHttpEndpointIsRejected() {
        let dict = Self.study(replacing: "ingestURL", with: "http://127.0.0.1:8089/api/v1/research/ingest")
        XCTAssertNil(ResearchStudyConfiguration.load(from: Self.info(dict)))
    }

    func testNonHttpsSchemesAreRejected() {
        for raw in [
            "ftp://ingest.example.org/upload",
            "ws://ingest.example.org/socket",
            "wss://ingest.example.org/socket",
            "file:///tmp/ingest",
            "data:application/json,{}",
            "ingest.example.org/api",
            "//ingest.example.org/api",
            "httpss://ingest.example.org/api"
        ] {
            let dict = Self.study(replacing: "ingestURL", with: raw)
            XCTAssertNil(ResearchStudyConfiguration.load(from: Self.info(dict)), raw)
        }
    }

    func testIPLiteralHostsAreRejectedEvenOverHttps() {
        for raw in [
            "https://127.0.0.1/ingest",
            "https://127.0.0.1:8089/api/v1/research/ingest",
            "https://10.0.0.5/ingest",
            "https://192.168.1.20:443/ingest",
            "https://[::1]/ingest",
            "https://[2001:db8::1]:8443/ingest",
            "https://127.1/ingest",
            "https://2130706433/ingest",
            "https://0x7f.0.0.1/ingest"
        ] {
            let dict = Self.study(replacing: "ingestURL", with: raw)
            XCTAssertNil(ResearchStudyConfiguration.load(from: Self.info(dict)), raw)
        }
    }

    func testIPLiteralErrorIsReported() {
        let dict = Self.study(replacing: "ingestURL", with: "https://127.0.0.1/ingest")
        XCTAssertThrowsError(try ResearchStudyConfiguration(dictionary: dict)) { error in
            XCTAssertEqual(error as? ResearchStudyConfiguration.ValidationError, .ipLiteralHost("127.0.0.1"))
        }
    }

    func testLocalhostIsRejected() {
        for raw in ["https://localhost/ingest", "https://LOCALHOST:8443/ingest", "https://api.localhost/ingest"] {
            let dict = Self.study(replacing: "ingestURL", with: raw)
            XCTAssertNil(ResearchStudyConfiguration.load(from: Self.info(dict)), raw)
        }
    }

    func testCredentialsInURLAreRejected() {
        let dict = Self.study(replacing: "ingestURL", with: "https://user:secret@ingest.example.org/ingest")
        XCTAssertNil(ResearchStudyConfiguration.load(from: Self.info(dict)))
    }

    func testMissingHostIsRejected() {
        let dict = Self.study(replacing: "ingestURL", with: "https:///ingest")
        XCTAssertNil(ResearchStudyConfiguration.load(from: Self.info(dict)))
    }

    func testURLValueMustBeAString() {
        let dict = Self.study(replacing: "ingestURL", with: 8089)
        XCTAssertNil(ResearchStudyConfiguration.load(from: Self.info(dict)))
    }

    func testValidateIngestURLDirectly() {
        XCTAssertNoThrow(try ResearchStudyConfiguration.validateIngestURL(URL(string: "https://ingest.example.org/x")!))
        XCTAssertThrowsError(try ResearchStudyConfiguration.validateIngestURL(URL(string: "http://ingest.example.org/x")!))
        XCTAssertThrowsError(try ResearchStudyConfiguration.validateIngestURL(URL(string: "https://127.0.0.1/x")!))
    }

    // MARK: - Required metadata

    func testEveryFieldIsRequired() {
        for key in Self.validDictionary.keys {
            let missing = Self.study(replacing: key, with: nil)
            XCTAssertNil(ResearchStudyConfiguration.load(from: Self.info(missing)), "missing \(key)")
            XCTAssertThrowsError(try ResearchStudyConfiguration(dictionary: missing))

            let blank = Self.study(replacing: key, with: "   ")
            XCTAssertNil(ResearchStudyConfiguration.load(from: Self.info(blank)), "blank \(key)")
        }
    }

    func testMissingFieldErrorNamesTheKey() {
        let dict = Self.study(replacing: "principalInvestigator", with: nil)
        XCTAssertThrowsError(try ResearchStudyConfiguration(dictionary: dict)) { error in
            XCTAssertEqual(error as? ResearchStudyConfiguration.ValidationError, .missingField("principalInvestigator"))
        }
    }

    // MARK: - WP-A.4 API base URL (optional, same https/DNS rule)

    func testAPIBaseURLIsOptional() throws {
        let config = try ResearchStudyConfiguration(dictionary: Self.validDictionary)
        XCTAssertNil(config.apiBaseURL, "no apiBaseURL key: the /v1 session client stays off")
    }

    func testAPIBaseURLIsParsed() throws {
        let dict = Self.study(replacing: ResearchStudyConfiguration.apiBaseURLKey, with: " https://api.example.org/asd ")
        let config = try ResearchStudyConfiguration(dictionary: dict)
        XCTAssertEqual(config.apiBaseURL, URL(string: "https://api.example.org/asd"))
        XCTAssertEqual(ResearchStudyConfiguration.apiBaseURLKey, "apiBaseURL")
    }

    func testAPIBaseURLMustBeHttpsWithADNSHost() {
        for bad in [
            "http://api.example.org",
            "https://127.0.0.1",
            "https://[::1]",
            "https://localhost",
            "https://user:pw@api.example.org",
            "ftp://api.example.org",
            "not a url",
            "   "
        ] {
            let dict = Self.study(replacing: ResearchStudyConfiguration.apiBaseURLKey, with: bad)
            XCTAssertThrowsError(try ResearchStudyConfiguration(dictionary: dict), bad)
            XCTAssertNil(ResearchStudyConfiguration.load(from: Self.info(dict)), "invalid apiBaseURL fails closed: \(bad)")
        }
        XCTAssertThrowsError(try ResearchStudyConfiguration(
            ingestURL: URL(string: "https://ingest.example.org/x")!,
            studyIdentifier: "S",
            irbProtocolId: "I",
            institution: "U",
            principalInvestigator: "P",
            consentVersion: "1",
            apiBaseURL: URL(string: "http://api.example.org")!
        ))
    }

    func testAPIBaseURLThatIsNotAStringFailsClosed() {
        let dict = Self.study(replacing: ResearchStudyConfiguration.apiBaseURLKey, with: 42)
        XCTAssertNil(ResearchStudyConfiguration.load(from: Self.info(dict)))
    }

    // MARK: - Consent document

    func testConsentDocumentIsBuiltFromConfiguration() throws {
        let config = try ResearchStudyConfiguration(dictionary: Self.validDictionary)
        let doc = ORKConsentDocument(configuration: config)
        XCTAssertEqual(doc.studyIdentifier, "EXAMPLE-STUDY-1")
        XCTAssertEqual(doc.irbProtocolId, "ETHICS-REF-42")
        XCTAssertEqual(doc.institution, "Example University")
        XCTAssertEqual(doc.principalInvestigator, "Example Investigator")
        XCTAssertEqual(doc.consentVersion, "2027.1")
        XCTAssertFalse(doc.sections.isEmpty)
        let allText = doc.sections.map { "\($0.title) \($0.summary) \($0.content)" }.joined(separator: " ")
        XCTAssertTrue(allText.contains("Example University"))
        XCTAssertTrue(allText.contains("ETHICS-REF-42"))
        for fictional in ["2026-088", "Vasquez", "CURV-ASD-GAZE-2026", "Center for Pediatric Neurodevelopment"] {
            XCTAssertFalse(allText.contains(fictional), fictional)
            XCTAssertFalse(doc.title.contains(fictional), fictional)
        }
    }
}
