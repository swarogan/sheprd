/// How many lines a list row's text may take: a setting, one by default
/// and three at most, so a row grows to show more of a reply but never
/// pushes the list into a wall of text.
public enum RowLines {
    /// The `UserDefaults` key the setting is kept under.
    public static let key = "rowLines"
    public static let choices = 1...3

    /// `stored` within the choices: an older or edited value cannot make a
    /// row blank or unbounded.
    public static func clamped(_ stored: Int) -> Int {
        min(max(stored, choices.lowerBound), choices.upperBound)
    }
}
