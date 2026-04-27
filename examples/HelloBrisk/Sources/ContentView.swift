import SwiftUI

struct ContentView: View {
    var body: some View {
        VStack(spacing: 12) {
            Text("HelloBrisk")
                .font(.system(size: 40, weight: .semibold, design: .rounded))
            Text("Built with brisk")
                .foregroundStyle(.secondary)
        }
        .frame(width: 520, height: 320)
    }
}
