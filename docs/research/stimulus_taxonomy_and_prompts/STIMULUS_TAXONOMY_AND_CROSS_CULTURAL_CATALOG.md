# Cross-Cultural & Multilingual ASD Stimulus Taxonomy and Generative Catalog

> **Document ID**: `DOC-RES-ASD-006`  
> **Target Subsystem**: `domains/asd_screening` & `apps/ios_asd_gaze_app`  
> **Clinical Grounding**: JAMA (2023) EarliPoint, NEJM AI (2024) SenseToKnow, eLife (2019) GeoPref  
> **Languages Covered**: English (`en`), Hindi (`hi`), Nepali (`ne`), Mandarin Chinese (`zh`)  
> **Ethnicities Covered**: South Asian (Indian/Nepali), East Asian (Chinese), Global Universal  

---

## 1. Clinical Rationale: Why Cross-Cultural & Linguistic Alignment Matters

Eye-tracking biomarkers for Autism Spectrum Disorder (ASD)—such as **Social Visual Engagement (SVE)**, **Eye Dwell Ratio**, and **Affective Mimicry**—rely heavily on the child feeling comfortable, socially referenced, and culturally attuned:

1. **Own-Race and Familiar Facial Biases in Early Infancy**:
   - Developmental research demonstrates that infants as young as 3–9 months exhibit perceptual familiarity with caregiver facial morphology and cultural prosody (tone of speech).
   - A South Asian toddler exposed primarily to Hindi or Nepali spoken at home may show attenuated orienting to an unfamiliar Western actor, not because of social impairment, but due to unfamiliar acoustic prosody.
2. **Audio-Visual Incongruence Hazard**:
   - Dubbing English videos with Hindi or Nepali destroys **lip synchrony (McGurk effect)**, confusing toddlers who naturally cross-reference auditory phonemes with mouth movements.
   - Native actors speaking native phonemes ensure genuine, biologically valid mouth-to-eye gaze saccades.
3. **Culture-Neutral Baselines**:
   - Animal motion (puppies, kittens, pandas) and mechanical toys (marble runs, spinning gears) serve as universal, culture-free control stimuli.

---

## 2. Age-Stratified Stimulus Architecture

```
┌──────────────────────────────────────────────────────────────────────────────────────────────────┐
│                               Stimulus Matrix & Taxonomy Architecture                            │
├───────────────────┬──────────────┬───────────────────────────────┬───────────────────────────────┤
│ Tier / Age Group  │ Duration     │ Stimulus Modality             │ Primary Biomarker Target      │
├───────────────────┼──────────────┼───────────────────────────────┼───────────────────────────────┤
│ Tier 1: 12–24 mo  │ 3s – 8s      │ Direct Eye Contact, Peek-a-boo│ SVE, Eye vs Mouth ratio,      │
│ (Early Toddler)   │              │ GeoPref Split Screen          │ Primary smile mimicry (AU12)  │
├───────────────────┼──────────────┼───────────────────────────────┼───────────────────────────────┤
│ Tier 2: 24–36 mo  │ 8s – 10s     │ Gaze Cueing, Joint Attention, │ Saccadic latency (ms),        │
│ (Older Toddler)   │              │ Mechanical Toy Contrast       │ Head orienting dynamics       │
├───────────────────┼──────────────┼───────────────────────────────┼───────────────────────────────┤
│ Tier 3: 36–60 mo  │ 8s – 12s     │ Turn-Taking, Expectancy Shift,│ Cognitive humor, flat affect  │
│ (Preschooler)     │              │ Cartoon Slapstick Animation   │ duration, language engagement │
└───────────────────┴──────────────┴───────────────────────────────┴───────────────────────────────┘
```

---

## 3. Multilingual Voice Scripts & Prompts by Ethnicity

All video generation prompts must adhere to the **Camera-Locked Rule**:
`"Static locked-off camera on tripod, eye-level, zero pan, zero zoom, soft diffused high-key studio lighting, solid pastel backdrop."`

---

### A. South Asian (Indian & Nepali) Cohort

#### 1. Hindi: Peek-a-boo & Social Warmth (`stim_hi_peekaboo_8s`)
* **Actor Description**: Friendly, warm Indian female teacher in a simple, modern pastel kurta, smiling warmly.
* **Native Vocal Script**:
  > *"छुपम छुपाई! कहाँ गए? ... (दोनों हाथों से चेहरा छुपाना) ... ता-दा! मैं यहाँ हूँ! कितनी प्यारी मुस्कान है!"*  
  > *(Romanized: "Chhupan chhupai! Kahan gaye? ... Ta-da! Main yahan hoon! Kitni pyaari muskaan hai!")*
* **AI Generative Prompt**:
  > *"Static tripod shot, locked camera. A warm, expressive Indian female teacher in her late 20s wearing a modern pastel yellow kurta against a minimalist warm cream studio wall. She looks directly into the camera lens with bright, caring eyes. At 2 seconds, she raises both hands to gently cover her face. At 4 seconds, she joyfully opens her hands, gasping with a radiant, genuine smile and gentle laugh, speaking 'Ta-da!'. Natural skin tones, catchlights in eyes, clean studio backdrop, 4k 60fps."*

#### 2. Hindi: Nursery Rhyme & Turn-Taking (`stim_hi_rhyme_10s`)
* **Actor Description**: Warm Indian male caregiver/educator sitting cross-legged.
* **Native Vocal Script**:
  > *"चंदा मामा दूर के, पुए पकाएं बूर के! आप भी गाओ? ... शाबाश!"*  
  > *(Romanized: "Chanda mama door ke, puye pakaayein boor ke! Aap bhi gaao? ... Shaabaash!")*
* **AI Generative Prompt**:
  > *"Static locked-off camera, eye-level. A friendly Indian male educator in a teal linen shirt against a soft mint background. He sings softly while making direct eye contact, taps his lap gently to the rhythm, pauses at 5 seconds with an expectant encouraging head-tilt, then beams a wide proud smile and claps twice. Ultra-sharp focus, professional softbox lighting, 60fps."*

#### 3. Nepali: Peek-a-boo & Warm Interaction (`stim_ne_peekaboo_8s`)
* **Actor Description**: Cheerful young Nepali woman with kind eyes and warm smile.
* **Native Vocal Script**:
  > *"लुकामारी! कहाँ गयो बाबु? ... (हातले मुख छोप्ने) ... ता-दा! म यहाँ छु! कति राम्रो!"*  
  > *(Romanized: "Lukamaari! Kahan gayo babu? ... Ta-da! Ma yahan chhu! Kati ramro!")*
* **AI Generative Prompt**:
  > *"Static eye-level camera on tripod. A cheerful Nepali young woman with natural warm smile sitting against a clean soft lavender wall. She looks straight into the camera. At 2.5 seconds, she playfully covers her face with both hands, peeks through her fingers, then opens them wide with a joyful exclamation, laughing warmly and leaning forward slightly. High-key diffused daylight, zero camera motion, cinematic realism, 60fps."*

#### 4. Nepali: Joint Attention & Surprise Discovery (`stim_ne_joint_attention_8s`)
* **Actor Description**: Expressive Nepali male actor.
* **Native Vocal Script**:
  > *"हेर त, उ त्यहाँ के आयो? एउटा पुतली! कस्तो रमाइलो!"*  
  > *(Romanized: "Hera ta, u tyahan ke aayo? Euta putali! Kasto ramailo!")*
* **AI Generative Prompt**:
  > *"Locked camera shot. An expressive young Nepali man in a casual navy sweater against a plain ivory background. He makes eye contact for 2.5 seconds, suddenly widens his eyes in surprise, turns his head 40 degrees to the upper-right corner of the frame, points with his index finger, and exclaims softly with an open smile, holding the point for 4 seconds. Crisp focus, smooth natural movement, 60fps."*

---

### B. East Asian (Mandarin Chinese) Cohort

#### 1. Mandarin: Peek-a-boo (捉迷藏) (`stim_zh_peekaboo_8s`)
* **Actor Description**: Warm Chinese kindergarten teacher with clear, friendly expressions.
* **Native Vocal Script**:
  > *"捉迷藏！躲在哪里呢？... (双手捂脸) ... 哒-哒！我在这里！你好呀！"*  
  > *(Pinyin: "Zhuō mí cáng! Duǒ zài nǎ lǐ ne? ... Dā-dā! Wǒ zài zhè lǐ! Nǐ hǎo ya!")*
* **AI Generative Prompt**:
  > *"Locked-off static tripod shot, eye level. A gentle, smiling East Asian woman in her late 20s wearing a clean pastel apricot knit sweater against a smooth light-gray backdrop. She looks directly at the camera with warm expressive eyes. At 2 seconds she covers her face with both palms, then spreads her fingers open at 4 seconds with a cheerful 'Ta-da!' smile and gentle nod. High-key diffused lighting, photorealistic, 60fps."*

#### 2. Mandarin: Nursery Rhyme & Joint Attention (`stim_zh_rhyme_10s`)
* **Native Vocal Script**:
  > *"小兔子乖乖，把门儿开开！快看，小兔子跑去那边了！"*  
  > *(Pinyin: "Xiǎo tù zi guāi guāi, bǎ mén er kāi kāi! Kuài kàn, xiǎo tù zi pǎo qù nà biān le!")*
* **AI Generative Prompt**:
  > *"Static locked camera. An expressive East Asian female teacher singing with subtle hand gestures against a plain soft sky-blue wall. At 4 seconds, her eyes follow an imaginary butterfly upward to the top-left corner of the screen, head turning gracefully while pointing left with a cheerful gasp. Sharp catchlights in eyes, professional studio quality, 60fps."*

---

### C. Universal / Culture-Neutral Stimuli (All Cohorts)

#### 1. Playful Puppies Tumbling (`stim_universal_puppies_8s`)
* **Prompt**: *"Static locked tripod camera, low angle. Two fluffy golden retriever puppies wrestling playfully on a neutral cream rug. Smooth natural animal movement, crisp focus, no humans, 60fps."*
* **Purpose**: Biological motion control (tests if gaze disinterest is human-specific or universal).

#### 2. Red Panda Cub Playing (`stim_universal_red_panda_8s`)
* **Prompt**: *"Locked camera, eye-level. An adorable fluffy red panda cub rolling over a smooth wooden log, swatting at a dangling leaf with curious round eyes. Solid warm beige background, clean organic motion, 60fps."*

#### 3. Kinetic Marble Run & Spinning Cogwheels (`stim_universal_gears_8s`)
* **Prompt**: *"Static 45-degree angle close-up. Vibrant wooden marble run with colorful interlocking gears in red, teal, and yellow. Smooth glass marbles rolling along zig-zag tracks, spinning mechanical wheels in continuous rhythmic motion. Solid gray table, high-contrast physics, 60fps."*

---

## 4. Auditory Orienting & Sound-Only Protocol ("Response to Name")

### Clinical Concept (Derived from SenseToKnow / NEJM AI 2024):
In early autism screening, one of the earliest and most specific behavioral markers is failure to orient or turn toward the caregiver when their name is called.

### Mobile Implementation with Front TrueDepth Camera:
1. **Screen State**: A calming, neutral pulsating circle in the screen center keeps the child looking forward.
2. **Audio Cue**:
   - At timestamp $t = 3.0\text{ s}$, the phone's stereo speaker plays a localized voice prompt in either the **Left Speaker** or **Right Speaker**:
     - English: *"Aarav! Look here!"*
     - Hindi: *"आरव! इधर देखो!"*
     - Nepali: *"आरभ! यता हेर त!"*
     - Chinese: *"浩然！看这边！"*
3. **TrueDepth Sensor Telemetry**:
   - Front camera continuously measures 3D Head Pose `yawAngleDegrees` at 60 Hz.
   - **Threshold**: Head turn $\ge 20^\circ$ toward the sound source within $1.5\text{ seconds}$.
   - **Biomarkers Computed**:
     - `auditory_orient_latency_ms`: Time from audio onset to head yaw threshold.
     - `auditory_orient_success`: Boolean flag indicating complete head rotation.

---

## 5. Summary of On-Demand Resource (ODR) Tag Hierarchy

```
App Store Hosted ODR Tags
├── pack_core_baseline        (Universal GeoPref & Peekaboo, always cached)
├── pack_age_12_24_en         (English early toddler video set)
├── pack_age_12_24_hi         (Hindi early toddler video set)
├── pack_age_12_24_ne         (Nepali early toddler video set)
├── pack_age_12_24_zh         (Mandarin early toddler video set)
├── pack_age_24_36_*          (Joint attention & gaze cueing sets)
├── pack_age_36_60_*          (Turn taking & humor sets)
└── pack_universal_animals    (Puppies, pandas, mechanical toys)
```
