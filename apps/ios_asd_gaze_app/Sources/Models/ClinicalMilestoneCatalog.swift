import Foundation

/// CDC / AAP surveillance catalog: 9–36 months × four developmental domains.
public enum ClinicalMilestoneCatalog {
    public static func all(completedAges: Set<Int> = [18]) -> [Milestone] {
        CDCSurveillanceAge.allCases.flatMap { age in
            DevelopmentalDomain.allCases.map { domain in
                make(age: age, domain: domain, completed: completedAges.contains(age.rawValue))
            }
        }
    }

    public static func make(age: CDCSurveillanceAge, domain: DevelopmentalDomain, completed: Bool) -> Milestone {
        let items = items(for: age, domain: domain).map { item in
            var copy = item
            copy.isCompleted = completed
            return copy
        }
        return Milestone(
            id: "ms_\(age.rawValue)_\(domain.shortLabel.lowercased())",
            ageMonths: age.rawValue,
            title: "\(age.rawValue)-month \(domain.shortLabel)",
            dueDateString: "CDC / AAP \(age.rawValue)-month well-child window",
            domain: domain,
            items: items,
            isCompleted: completed
        )
    }

    private static func item(_ title: String, look: String, prompt: String, consult: String) -> MilestoneChecklistItem {
        MilestoneChecklistItem(title: title, whatToLookFor: look, howToPrompt: prompt, whenToConsult: consult)
    }

    private static func items(for age: CDCSurveillanceAge, domain: DevelopmentalDomain) -> [MilestoneChecklistItem] {
        switch (age, domain) {
        case (.months9, .socialEmotional):
            return [
                item("Stranger caution with familiar caregiver check-in",
                     look: "May cling or look to you when a new adult approaches, then return to play.",
                     prompt: "Have a trusted adult hold the child while a new person sits nearby and smiles.",
                     consult: "No preference for familiar caregivers, or no social referencing by 12 months.")
            ]
        case (.months9, .languageCommunication):
            return [
                item("Responds to name and simple gestures",
                     look: "Turns, pauses, or vocalizes when called; may wave or reach.",
                     prompt: "Call the child's name from 1–2 meters during play, then wait 3 seconds.",
                     consult: "No consistent name response after several well-child visits.")
            ]
        case (.months9, .cognitive):
            return [
                item("Object permanence search",
                     look: "Looks for a toy you hide under a cloth.",
                     prompt: "Cover a favorite toy and wait; praise looking or reaching.",
                     consult: "No search for hidden objects by 12 months.")
            ]
        case (.months9, .sensoryMotor):
            return [
                item("Sits and reaches with stable gaze",
                     look: "Sits without support and tracks a toy across midline.",
                     prompt: "Slowly move a silent toy left to right at eye level.",
                     consult: "Persistent gaze aversion or inability to sit by this window — discuss with pediatrician.")
            ]
        case (.months12, .socialEmotional):
            return [
                item("Joint attention check-ins",
                     look: "Looks from toy to your face and back to share interest.",
                     prompt: "Hold an interesting object, look at it, then at the child, then back.",
                     consult: "No shared looks to comment by 15–18 months.")
            ]
        case (.months12, .languageCommunication):
            return [
                item("Proto-declarative reach or point",
                     look: "Shows or reaches toward something interesting, not only to request food.",
                     prompt: "Point to a distant light or pet and say 'look' without handing an object.",
                     consult: "Only imperative (requesting) gestures with no sharing of interest.")
            ]
        case (.months12, .cognitive):
            return [
                item("Imitates simple actions",
                     look: "Copies clap, bang, or peek-a-boo after you model once.",
                     prompt: "Do one action, pause, invite 'your turn'.",
                     consult: "No imitation of familiar actions by 15 months.")
            ]
        case (.months12, .sensoryMotor):
            return [
                item("Orients to quiet speech and everyday sounds",
                     look: "Turns toward voice or household sounds without distress.",
                     prompt: "Call name from behind during calm play.",
                     consult: "No orientation to voice, or extreme distress to ordinary sounds.")
            ]
        case (.months18, .socialEmotional):
            return [
                item("Reciprocal peek-a-boo and social smile",
                     look: "Sustains back-and-forth smiling and eye contact during a short game.",
                     prompt: "Play peek-a-boo for 30 seconds; wait for the child to restart the game.",
                     consult: "No reciprocal games or social smile by 18 months.")
            ]
        case (.months18, .languageCommunication):
            return [
                item("Declarative pointing to share",
                     look: "Points to show you something interesting without needing the object.",
                     prompt: "Point to a plane or bird; wait for the child to look then look back at you.",
                     consult: "No pointing to share interest — a CDC social-communication flag.")
            ]
        case (.months18, .cognitive):
            return [
                item("Simple pretend with everyday objects",
                     look: "Feeds a doll or talks on a toy phone.",
                     prompt: "Offer a cup and doll; model one sip, then wait.",
                     consult: "No pretend actions by 24 months.")
            ]
        case (.months18, .sensoryMotor):
            return [
                item("Follows a point to a distant target",
                     look: "Looks where you point across the room (M-CHAT-R item 1).",
                     prompt: "Point clearly, say 'look', and wait before repeating.",
                     consult: "Does not follow a point after several tries in a quiet room.")
            ]
        case (.months24, .socialEmotional):
            return [
                item("Notices others' affect",
                     look: "Looks concerned or offers a toy if a peer or caregiver is upset.",
                     prompt: "Narrate feelings during stories: 'The bear is sad.'",
                     consult: "No response to others' distress by 30 months.")
            ]
        case (.months24, .languageCommunication):
            return [
                item("Two-word phrases to comment or request",
                     look: "Combines words ('more milk', 'daddy go') not only echoing.",
                     prompt: "Pause during routines so the child can fill in a word.",
                     consult: "Fewer than 50 words or no two-word combinations.")
            ]
        case (.months24, .cognitive):
            return [
                item("Pretend sequences",
                     look: "Feeds then puts doll to bed, or stirs then 'eats'.",
                     prompt: "Set out two related toys and model a two-step sequence.",
                     consult: "Play remains only sensorimotor (mouthing, lining up) with no pretend.")
            ]
        case (.months24, .sensoryMotor):
            return [
                item("Social looking preferred over repeating patterns",
                     look: "During screen play, looks more at faces than spinning geometric clips when both are available.",
                     prompt: "Sit together for a short face-to-face song, then a brief pattern video; notice looking.",
                     consult: "Marked preference for repeating visual patterns with little social looking — discuss screening.")
            ]
        case (.months30, .socialEmotional):
            return [
                item("Shows objects to share enjoyment",
                     look: "Brings toys to you to look at together.",
                     prompt: "Act delighted when shown something; wait for another offer.",
                     consult: "Never shows objects to share by 30 months.")
            ]
        case (.months30, .languageCommunication):
            return [
                item("Uses words for comments, not only needs",
                     look: "Names pictures or events ('dog!', 'uh-oh') without requesting.",
                     prompt: "Look at a book and pause before turning the page.",
                     consult: "Language used only to request, never to comment.")
            ]
        case (.months30, .cognitive):
            return [
                item("Solves simple problems with help",
                     look: "Tries a shape sorter or container lid, looks to you if stuck.",
                     prompt: "Give a slightly hard lid; wait, then point to the solution.",
                     consult: "No problem-solving attempts or help-seeking gaze.")
            ]
        case (.months30, .sensoryMotor):
            return [
                item("Tolerates mixed sensory play with support",
                     look: "Can join brief messy or noisy play with a familiar adult nearby.",
                     prompt: "Offer dry beans or play-dough for 2 minutes with an escape option.",
                     consult: "Extreme aversion that prevents daily care (bath, clothes, meals).")
            ]
        case (.months36, .socialEmotional):
            return [
                item("Cooperative play turns",
                     look: "Takes turns in a simple game with a peer or adult.",
                     prompt: "Roll a ball back and forth; count turns out loud.",
                     consult: "No turn-taking interest in familiar games.")
            ]
        case (.months36, .languageCommunication):
            return [
                item("Conversational back-and-forth",
                     look: "Adds a related idea after you speak, not only repeating.",
                     prompt: "Ask a choice question and wait 5 seconds.",
                     consult: "Speech is mostly echolalia or unused for social conversation.")
            ]
        case (.months36, .cognitive):
            return [
                item("Flexible pretend roles",
                     look: "Assigns roles ('you baby, me mama') or uses one object as another.",
                     prompt: "Offer a block as a 'phone' and wait.",
                     consult: "Play stays rigid or solely repetitive.")
            ]
        case (.months36, .sensoryMotor):
            return [
                item("Stable social visual attention in conversation",
                     look: "Looks at speaker's face enough to stay engaged in a 1–2 minute chat.",
                     prompt: "Sit at eye level during a snack conversation.",
                     consult: "Cannot sustain any social looking during everyday talk — share gaze-app trends with clinician.")
            ]
        }
    }
}
