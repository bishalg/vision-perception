---
name: ux-writing
description: Principles, heuristics, and guidelines for UX writing, microcopy, error messaging, and strategic feature naming across interfaces and platforms. Use when naming features, writing button labels, empty states, error messages, onboarding flows, or auditing interface copy for clarity, tone, consistency, and user agency.
---

# UX Writing & Strategic Naming

The words chosen for an app are an essential part of its user experience. Designing through the lens of language—from naming core features to formatting error messages—helps create a cohesive and intuitive environment. The way an interface communicates is a direct reflection of broader design principles, ensuring that software feels crafted, simple, and respectful of the user's agency.

---

## 1. Core Principles: Writing as a Design Element

Great writing is an exercise in intentional design. Treat microcopy as an architectural component of the UI.

- **Simplicity**: Concise language and clear visual/textual hierarchy help achieve simplicity, ensuring the user can instantly answer: *What should I pay attention to?* and *How do I interact with this?*
- **Agency**: Offering clear, honest choices through text puts users in full control of their experience. Avoid manipulative framing, dark patterns, or vague confirmations.
- **Craft**: Paying meticulous attention to the details of UI writing inspires confidence and shows the user that their experience was built with intentional care.

---

## 2. Strategic Naming

Naming decisions happen at every level—from primary navigation tabs and feature suites to settings toggles. These choices accumulate to shape how someone perceives and trusts an app. The best names speak to the person using the feature rather than just exposing the underlying engineering or data model.

### Key Evaluation Criteria

A successful name meets three fundamental criteria:

1. **Belongs**: The name fits naturally into the product ecosystem, matches user mental models, and lives harmoniously among existing named elements.
2. **Expectations**: The name builds trust by delivering exactly what the user predicts they will find. Eliminate ambiguity, especially in sensitive contexts like finance, privacy, health, and security.
3. **Works Everywhere**: The name translates seamlessly across different languages, international markets, cultural contexts, and device form factors.

### The "Think, Feel, Do" Exercise

When brainstorming and establishing names:

1. **Identify the Target Audience**: Define specifically who is interacting with the feature.
2. **Define Outcomes**:
   - **Think**: What mental model or concept should the user form?
   - **Feel**: What emotional state should the feature evoke (e.g., confident, relieved, empowered)?
   - **Do**: What specific action is the user trying to accomplish?
3. **Divergent & Convergent Brainstorming**: Generate candidate names without filtering, then cluster them by recurring themes.

### Conversational Testing

Test potential names by placing them into natural, spoken sentences:
- *"Hey, check out [Name]."*
- *"Where can I find [Name]?"*
- *"I used [Name] to update my profile."*

If the name sounds awkward, overly technical, or robotic in casual conversation, refine it.

### Approaches to Naming

| Approach | Description | Ideal Use Case | Examples |
| :--- | :--- | :--- | :--- |
| **Descriptive** | Uses industry-standard, neutral, self-explanatory terms. Maximizes clarity and trust. | Utility tools, settings, financial data, functional screens. | *Balance*, *Visited Places*, *Export History*, *Saved Addresses* |
| **Emotional** | Evocative, warm, narrative-driven terms. | Features where the user's relationship is affective or memory-focused. | *Memories*, *Highlights*, *Reflections*, *Focus* |
| **Action-Oriented** | Starts with or centers around active verbs. Puts the user in direct control. | Functional capabilities, enhancements, quick tools. | *Enhance Dialogue*, *Translate*, *Organize*, *Clean Up* |

---

## 3. Interface Writing & Copy Guidelines

Once a voice is established to reflect the product's values, tone must adapt dynamically to the situational context (e.g., celebratory on milestone completion, calm and precise during errors).

### Clarity and Action

- **Plain Language**: Choose plain, easily understood terms. Strip away unnecessary words and technical jargon.
- **Active Voice**: Keep sentences active and direct (*"Download your statement"* vs. *"Your statement can be downloaded"*).
- **Verb-Led Button Labels**: Ensure buttons and action triggers start with verbs indicating the precise outcome (*"Save Changes"*, *"Delete Account"*, *"Send Invite"*).
- **Clarity Over Cleverness**: A straightforward label is always superior to playful ambiguity (*"Send"* > *"Let's do it!"*; *"Sign Out"* > *"See you later!"*).

### Building Language Patterns & Consistency

- **Capitalization Hierarchy**: Standardize across components (e.g., Title Case for navigation titles and buttons, Sentence case for body copy, tooltips, and descriptions). Apply rules rigorously.
- **Consistent Multi-Step Terminology**: Maintain predictable labels across onboarding and wizards (*"Get Started"* $\rightarrow$ *"Next"* $\rightarrow$ *"Done"* / *"Submit"*). Never alternate between synonymous labels (like switching between *"Continue"* and *"Next"* arbitrarily).

### Pronouns and Perspective

- **Minimize Possessive Pronouns**: Drop unnecessary "Your" / "My" to reduce cognitive load and visual clutter (*"Favorites"* instead of *"Your Favorites"*; *"Profile"* instead of *"My Profile"*).
- **Avoid the Royal "We"**: Avoid referring to the company or system as "we" (*"We couldn't find..."* $\rightarrow$ *"No results found"*), which blurs the boundary of agency and can feel patronizing.

### Contextual and Device Considerations

- **Interaction Verbs**: Match the physical modality (*"Tap"* for touchscreens, *"Click"* for desktop/web, *"Press"* for hardware buttons/remotes).
- **Screen Real Estate**:
  - *Small displays (Watch/Wearables)*: Ultra-concise, single-line messages, essential verbs only.
  - *Ten-foot UI (TV/Consoles)*: Large glanceable headlines, formatted for rapid reading from a distance.
  - *Mobile/Desktop*: Scannable subheadings, scannable bullet points, immediate primary actions.

---

## 4. Handling Errors, Warnings, and Empty States

### Error Messages

Error states must never frustrate or blame the user.

- **Placement**: Display the error inline, directly adjacent to the input or element that caused the issue.
- **Structure**:
  1. State what happened plainly without jargon or error codes.
  2. Explain how to resolve it immediately.
- **No Blame**: Avoid accusatory phrases like *"You entered an invalid email"*. Use *"Enter a valid email address (e.g., name@example.com)"*.
- **Avoid Sarcasm / Fake Sympathy**: Do not use insincere interjections like *"Oops!"*, *"Uh-oh!"*, or *"Whoops!"*. Keep the tone calm, helpful, and professional.

### Empty States

- Never leave a blank screen or a dead end.
- State what belongs in the space and why it is currently empty.
- Provide a direct, primary action button to get started (*"No Projects Yet — Create your first project to start tracking tasks"* $\rightarrow$ `[Create Project]`).

---

## 5. UX Copy Review Matrix

When reviewing or drafting copy, evaluate against this reference matrix:

| Element | Poor / Discouraged | Recommended | Rationale |
| :--- | :--- | :--- | :--- |
| **Action Button** | *OK*, *Submit*, *Let's go!* | *Save Changes*, *Create Account*, *Send Message* | Verb-led, specific outcome |
| **Error Title** | *Oops! Something went wrong.* | *Unable to Save Document* | Direct, objective, dignified |
| **Error Body** | *You entered an incorrect password format.* | *Password must include at least 8 characters and 1 number.* | Explains how to fix without blaming |
| **Empty State** | *Nothing here.* | *No starred items. Star files to access them quickly offline.* | Explains purpose and action |
| **Destructive Action** | *Are you sure? [Yes] [No]* | *Delete Report? This action cannot be undone. [Cancel] [Delete Report]* | Explicit consequences, specific buttons |
| **Settings Label** | *Your Notification Preferences* | *Notifications* | Concise, drops possessive pronoun |
| **Feature Name** | *Dynamic Cloud Sync Pipeline v2* | *Instant Sync* | Action/benefit-focused, no internal tech jargon |
