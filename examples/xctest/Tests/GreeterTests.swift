import XCTest
@testable import XCTestExample

final class GreeterTests: XCTestCase {
    func testMessage() {
        XCTAssertEqual(Greeter.message, "Hello from XCTestExample")
    }
}
