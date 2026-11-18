import SwiftUI

/// Shows one mockup, chosen by launch arguments:
/// `-screen home -variant A -state 0`. Without them, a list of all.
@main
struct MockupApp: App {
    var body: some Scene {
        WindowGroup {
            let defaults = UserDefaults.standard
            if let screen = defaults.string(forKey: "screen") {
                Mockup(screen: screen, variant: defaults.string(forKey: "variant") ?? "A",
                       state: defaults.integer(forKey: "state"))
            } else {
                Catalog()
            }
        }
    }
}

struct Mockup: View {
    var screen: String
    var variant: String
    var state: Int

    var body: some View {
        switch (screen, variant) {
        case ("pair", "A"): PairRadar(state: state)
        case ("pair", "B"): PairCard(state: state)
        case ("pair", "C"): PairSteps(state: state)
        case ("home", "A"): HomeGrouped()
        case ("home", "B"): HomeAttention()
        case ("home", "C"): HomeFiltered()
        case ("chat", "A"): ChatFocus()
        case ("chat", "B"): ChatDocument()
        case ("chat", "C"): ChatBubbles()
        case ("lock", "A"): LockQuestion()
        case ("lock", "B"): LockStepStack()
        case ("lock", "C"): IslandExpanded()
        case ("computers", "A"): ComputersGrouped()
        case ("computers", "B"): ComputersCarousel()
        case ("computers", "C"): ComputersTree()
        case ("new", "A"): NewWorkspaceForm()
        case ("new", "B"): NewWorkspacePicker()
        case ("new", "C"): NewWorkspacePrompt()
        case ("session", "A"): SessionList()
        case ("session", "B"): SessionPanel()
        case ("session", "C"): SessionCards()
        case ("term", "A"): TerminalFull()
        case ("term", "B"): TerminalPanes()
        case ("term", "C"): TerminalLine()
        case ("settings", "A"): SettingsGrouped()
        case ("settings", "B"): SettingsPerComputer()
        case ("settings", "C"): SettingsStatus()
        case ("ipad", "A"): IPadColumns()
        case ("ipad", "B"): IPadInspector()
        case ("ipad", "C"): IPadBoard()
        case ("connect", "Now"): ConnectionNow(state: state)
        case ("connect", "A"): ConnectionSubtitle(state: state)
        case ("connect", "B"): ConnectionCapsule(state: state)
        case ("connect", "C"): ConnectionToast(state: state)
        case ("connect", "D"): ConnectionQuiet(state: state)
        case ("sync", "A"): SyncEdge(state: state)
        case ("sync", "B"): SyncStrip(state: state)
        case ("sync", "C"): SyncTab(state: state)
        case ("sync", "D"): SyncIsland(state: state)
        case ("spaces", "A"): SpacesSidebar(state: state)
        case ("spaces", "B"): SpacesTerminal(state: state)
        case ("spaces", "C"): SpacesOneComputer(state: state)
        case ("spaces", "D"): SpacesFolded(state: state)
        case ("spaces", "Mix"): SpacesMix(state: state)
        case ("forget", "A"): ForgetPage(state: state)
        case ("forget", "B"): ForgetHeaderMenu(state: state)
        case ("forget", "C"): ForgetUnpairedCard(state: state)
        case ("forget", "D"): ForgetEditList(state: state)
        case ("newtab", "A"): NewTabGrid()
        case ("newtab", "B"): NewTabMenu(state: state)
        case ("newtab", "C"): NewTabQuick(state: state)
        case ("newtab", "D"): NewTabRowSheet(state: state)
        case ("tabs", "Now"): TabListNow()
        case ("tabs", "A"): TabListPromptTitle()
        case ("tabs", "B"): TabListCentered()
        case ("tabs", "C"): TabListThreeLines()
        case ("tabs", "D"): TabListOneLine()
        case ("counts", "A"): HomeCountsChips(state: state)
        case ("counts", "B"): HomeCountsCard(state: state)
        case ("counts", "C"): HomeCountsTiles(state: state)
        case ("counts", "D"): HomeCountsLine(state: state)
        case ("ask", "A"): QuestionScrollSheet(state: state)
        case ("ask", "B"): QuestionListSheet(state: state)
        case ("ask", "C"): QuestionInlineCard(state: state)
        case ("ask", "D"): QuestionPages(state: state)
        case ("voice", "A"): VoiceSheetLayout(state: state)
        case ("voice", "B"): VoiceChecklist(state: state)
        case ("voice", "C"): VoiceAmbient(state: state)
        case ("voice", "D"): VoiceSentence(state: state)
        case ("prhead", "A"): PRHeadCard(state: state)
        case ("prhead", "B"): PRHeadRows(state: state)
        case ("prhead", "C"): PRHeadBanner(state: state)
        case ("prhead", "D"): PRHeadBar(state: state)
        case ("prdetail", "A"): PRDetailSections(state: state)
        case ("prdetail", "B"): PRDetailTabs(state: state)
        case ("prdetail", "C"): PRDetailTimeline(state: state)
        case ("prdetail", "D"): PRDetailGrid(state: state)
        case ("prlist", "A"): PRListSwitch(state: state)
        case ("prlist", "B"): PRListSection(state: state)
        case ("prlist", "C"): PRListRepos(state: state)
        case ("prlist", "D"): PRListCards(state: state)
        case ("ghsettings", "A"): GHSettingsRows(state: state)
        case ("ghsettings", "B"): GHSettingsPage(state: state)
        case ("ghsettings", "C"): GHSettingsCards(state: state)
        case ("ghsettings", "D"): GHSettingsByComputer(state: state)
        case ("ghsignin", "A"): GHSignInSheet(state: state)
        case ("ghsignin", "B"): GHSignInSteps(state: state)
        case ("ghsignin", "C"): GHSignInInline(state: state)
        case ("agentpr", _): AgentPRHome(variant: variant)
        default: Text("No mockup \(screen) \(variant)")
        }
    }
}

struct Catalog: View {
    private let entries: [(String, String, Int)] = [
        ("pair", "A", 0), ("pair", "A", 1), ("pair", "B", 0), ("pair", "B", 1), ("pair", "C", 0), ("pair", "C", 1),
        ("home", "A", 0), ("home", "B", 0), ("home", "C", 0),
        ("chat", "A", 0), ("chat", "B", 0), ("chat", "C", 0),
        ("lock", "A", 0), ("lock", "B", 0), ("lock", "C", 0),
        ("computers", "A", 0), ("computers", "B", 0), ("computers", "C", 0),
        ("new", "A", 0), ("new", "B", 0), ("new", "C", 0),
        ("session", "A", 0), ("session", "B", 0), ("session", "C", 0),
        ("term", "A", 0), ("term", "B", 0), ("term", "C", 0),
        ("settings", "A", 0), ("settings", "B", 0), ("settings", "C", 0),
        ("ipad", "A", 0), ("ipad", "B", 0), ("ipad", "C", 0),
        ("connect", "Now", 0), ("connect", "A", 0), ("connect", "B", 0), ("connect", "C", 0), ("connect", "D", 0),
        ("spaces", "A", 0), ("spaces", "A", 1), ("spaces", "B", 0), ("spaces", "B", 1),
        ("spaces", "C", 0), ("spaces", "C", 1), ("spaces", "D", 0), ("spaces", "D", 1), ("spaces", "Mix", 0), ("spaces", "Mix", 1),
        ("newtab", "A", 0), ("newtab", "B", 0), ("newtab", "B", 1), ("newtab", "C", 0), ("newtab", "C", 1), ("newtab", "D", 0), ("newtab", "D", 1),
        ("tabs", "Now", 0), ("tabs", "A", 0), ("tabs", "B", 0), ("tabs", "C", 0), ("tabs", "D", 0),
        ("counts", "A", 0), ("counts", "A", 1), ("counts", "B", 0), ("counts", "B", 1),
        ("counts", "C", 0), ("counts", "C", 1), ("counts", "D", 0), ("counts", "D", 1),
        ("ask", "A", 0), ("ask", "A", 1), ("ask", "B", 0), ("ask", "B", 1),
        ("ask", "C", 0), ("ask", "C", 1), ("ask", "D", 0), ("ask", "D", 1),
        ("voice", "A", 0), ("voice", "A", 1), ("voice", "A", 2), ("voice", "A", 3),
        ("voice", "B", 0), ("voice", "B", 1), ("voice", "B", 2), ("voice", "B", 3),
        ("voice", "C", 0), ("voice", "C", 1), ("voice", "C", 2), ("voice", "C", 3),
        ("voice", "D", 0), ("voice", "D", 1), ("voice", "D", 2), ("voice", "D", 3),
        ("prhead", "A", 0), ("prhead", "A", 1), ("prhead", "B", 0), ("prhead", "B", 1),
        ("prhead", "C", 0), ("prhead", "C", 1), ("prhead", "D", 0), ("prhead", "D", 1),
        ("prdetail", "A", 0), ("prdetail", "A", 1), ("prdetail", "B", 0), ("prdetail", "B", 1),
        ("prdetail", "C", 0), ("prdetail", "C", 1), ("prdetail", "D", 0), ("prdetail", "D", 1),
        ("prlist", "A", 0), ("prlist", "A", 1), ("prlist", "B", 0), ("prlist", "B", 1),
        ("prlist", "C", 0), ("prlist", "C", 1), ("prlist", "D", 0), ("prlist", "D", 1),
        ("ghsettings", "A", 0), ("ghsettings", "A", 1), ("ghsettings", "A", 2), ("ghsettings", "A", 3),
        ("ghsettings", "B", 0), ("ghsettings", "B", 1), ("ghsettings", "C", 0), ("ghsettings", "C", 1),
        ("ghsettings", "D", 0), ("ghsettings", "D", 1),
        ("ghsignin", "A", 0), ("ghsignin", "A", 1), ("ghsignin", "A", 2), ("ghsignin", "A", 3), ("ghsignin", "A", 4),
        ("ghsignin", "B", 0), ("ghsignin", "B", 1), ("ghsignin", "B", 2), ("ghsignin", "B", 3), ("ghsignin", "B", 4),
        ("ghsignin", "C", 0),
        ("ghsignin", "C", 1), ("ghsignin", "C", 2), ("ghsignin", "C", 3), ("ghsignin", "C", 4),
        ("agentpr", "A", 0), ("agentpr", "B", 0), ("agentpr", "C", 0),
    ]

    var body: some View {
        NavigationStack {
            List {
                ForEach(Array(entries.enumerated()), id: \.offset) { _, entry in
                    NavigationLink("\(entry.0) \(entry.1) \(entry.2)") {
                        Mockup(screen: entry.0, variant: entry.1, state: entry.2)
                    }
                }
            }
            .navigationTitle("Mockups")
        }
    }
}
