import Foundation
import Testing
@testable import SheprdRemoteCore

/// A question asked with choices: read whole, answered with several
/// choices or the person's own words.
struct AskedQuestionTests {
    @Test func decodesAQuestionBesideItsOptions() throws {
        let json = #"""
        {"id": 4, "title": "Which details?", "options": [{"id": "reject", "kind": "reject_once", "name": "Answer in the chat"}],
         "question": {"header": "Row details", "multiple": true, "text": "Which details should a row show?",
                      "options": [{"label": "Running command", "description": "The process."}, {"label": "Git branch"}]}}
        """#
        let permission = try JSONDecoder().decode(Permission.self, from: Data(json.utf8))
        let question = try #require(permission.question)
        #expect(question.header == "Row details")
        #expect(question.multiple)
        #expect(question.options.map(\.label) == ["Running command", "Git branch"])
        #expect(question.options[0].description == "The process.")
        #expect(question.options[1].description == nil)
    }

    @Test func aPermissionFromAnOlderProgramHasNoQuestion() throws {
        let json = #"{"id": 4, "title": "Run it", "options": []}"#
        #expect(try JSONDecoder().decode(Permission.self, from: Data(json.utf8)).question == nil)
    }

    @Test func severalAnswersKeepTheOfferedOrderThenOwnWords() {
        let question = AskedQuestion(text: "Pick", multiple: true, options: [.init(label: "A"), .init(label: "B"), .init(label: "C")])
        #expect(question.answers(chosen: ["C", "A"], own: "  ") == ["A", "C"])
        #expect(question.answers(chosen: ["B"], own: " mine ") == ["B", "mine"])
        #expect(question.answers(chosen: [], own: "") == [])
    }

    @Test func ownWordsReplaceTheOneChoice() {
        let question = AskedQuestion(text: "Pick", options: [.init(label: "A"), .init(label: "B")])
        #expect(question.answers(chosen: ["B"], own: "") == ["B"])
        #expect(question.answers(chosen: ["B"], own: "neither") == ["neither"])
    }

    @Test func answeringIsWrittenAsTheProgramReadsIt() throws {
        let command = ChatCommand.answerQuestion(permission: PermissionId(4), answers: ["A", "mine"])
        let written = try JSONSerialization.jsonObject(with: JSONEncoder().encode(command)) as? [String: Any]
        #expect(written?["command"] as? String == "answer_question")
        #expect(written?["answers"] as? [String] == ["A", "mine"])
        #expect(written?["permission"] as? Int == 4)
        #expect(Expected(command) == .answer(PermissionId(4)))
    }
}
