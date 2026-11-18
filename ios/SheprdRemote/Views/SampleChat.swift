#if DEBUG
import SheprdRemoteCore

// A made-up chat for the simulator loop's `-sampleChat YES`, so screenshots
// of the chat screen show no one's real prompts. No computer sends it.

extension PaneKey {
    static let sample = PaneKey(device: "sample", pane: "sample")
}

extension Transcript {
    /// An earlier turn, then a prompt longer than three lines and an answer
    /// long enough to scroll it away.
    static var sample: Transcript {
        var transcript = Transcript(agent: "Claude Code")
        transcript.status = .idle
        transcript.entries = [
            Entry(id: 1, kind: .user(text: "Add a dark mode toggle to the settings screen.", files: 0)),
            Entry(id: 2, kind: .agent(text: "Done: Settings has an Appearance row with System, Light, and Dark.")),
            Entry(id: 3, kind: .user(text: """
            The weather widget shows yesterday's forecast after the phone wakes up. \
            Find out why the timeline doesn't refresh, fix it so the widget reloads \
            when the app gets new data in the background, and add a test that covers \
            the stale case. Keep the refresh budget under the system's daily limit.
            """, files: 0)),
            Entry(id: 4, kind: .agent(text: """
            ## Why it was stale

            The widget's timeline ends at midnight with a `.never` policy, so \
            WidgetKit only reloads it when the app asks. The background refresh \
            task fetched new forecasts into the shared container but never called \
            `WidgetCenter.shared.reloadTimelines(ofKind:)`.

            ## The fix

            - The refresh task now reloads the forecast widget after it saves new data.
            - It only reloads when the forecast changed, comparing the saved \
            `Forecast.revision`, which keeps the reloads well inside the daily budget.
            - The timeline policy is `.after(nextHour)`, so a missed background \
            refresh still recovers within the hour.

            ## The test

            `ForecastWidgetTests.testReloadsAfterBackgroundFetch` saves a forecast, \
            runs the refresh task against a stub fetcher that returns a newer one, \
            and checks the timeline provider returns the new entry. A second test \
            checks that an unchanged forecast does not ask for a reload.

            ## Checked

            - `swift test`: 148 tests pass.
            - In the simulator, after **Debug › Simulate Background Fetch**, \
            the widget showed today's forecast within a second.
            - Over a day of background fetches the widget asked for 9 reloads, \
            against a budget of 40 to 70.
            """)),
        ]
        return transcript
    }
}
#endif
