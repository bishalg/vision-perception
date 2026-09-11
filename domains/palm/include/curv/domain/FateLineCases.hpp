#pragma once

#include <array>
#include <string>
#include <string_view>
#include <vector>

namespace CurvEngine::domain {

// Detection priority: P0 = must for robust fate (present/stops/broken/age),
// P1 = branching/multiplicity, P2 = shape marks, P3 = future multi-mount fusion.
enum class FatePriority : int { P0 = 0, P1 = 1, P2 = 2, P3 = 3 };

// Figure groups segregated from the palmistindian.com fate-line blog so each
// image maps to exactly one detection primitive. Group ids are stable for
// future tracking docs and fixture manifests.
enum class FateFigureGroup : int {
    Presence = 0,    // present / absent / no-fate
    Origin = 1,      // moon / life-joint / headline-origin / venus start
    Course = 2,      // short / long / broken / double / Y / fork / branches
    Termination = 3, // stops heart / stops head / merged head / cross sun / cross life
    Marks = 4,       // island-fish / triangle / cross / square / saturn-mercury grill
    Age = 5,         // 28 / 33 / 35 / 53 mapping
    Context = 6,     // 15% weight / luck-career alias / govt-job / beggar-billionaire
};

struct FateCaseDef {
    std::string_view id;          // stable snake_case id, e.g. "stops_at_head_35"
    FateFigureGroup group;        // figure group for segregated image sets
    std::string_view blog_anchor; // TOC anchor on the source article
    std::string_view image_url;   // hotlink reference (not vendored; see data/fixtures/palm_fate/)
    std::string_view image_file;  // canonical basename for local manifest, e.g. "fate-line-broken-1024x606.jpg"
    std::string_view primitive;   // CV primitive in canonical 512 space
    std::string_view use;         // interpretation trigger for report layer
    FatePriority priority;        // implementation order
    bool implemented{false};      // future tick-off flag
};

// Segregated array of every figure that can appear in a fate detection.
// Source: https://www.palmistindian.com/fate-line-money-line-in-palmistry-destiny-luck/
// (Manish Paliwal, Oct 2020, 32 types). Fate is optional: absent/faint is a
// valid `no_fate` result and must not fail the pipeline.
inline std::vector<FateCaseDef> AllFateCases() {
    return {
        // ---- Group 0: Presence ----
        {"present", FateFigureGroup::Presence, "#Beggars_and_Billionaire_do_have_a_fate_line",
         "https://palmistindian.com/wp-content/uploads/2020/10/fate-line-present-or-abscent-1024x606.jpg",
         "fate-line-present-or-abscent-1024x606.jpg", "chain len>40px && conf>0.55 else absent",
         "gate: !present -> status no_fate (billionaire may have none)", FatePriority::P0, true},
        {"no_fate", FateFigureGroup::Presence, "#Palm_reading_no_fate_line",
         "https://palmistindian.com/wp-content/uploads/2020/10/no-fate-line-1024x606.jpg", "no-fate-line-1024x606.jpg",
         "fate_present==false && mounts good -> unlimited possibilities", "positive if Mercury/Mars/Venus good",
         FatePriority::P0, true},
        // ---- Group 1: Origin ----
        {"from_moon", FateFigureGroup::Origin, "#Fate_line_starting_from_mount_of_moon",
         "https://palmistindian.com/wp-content/uploads/2020/11/fate-line-starts-from-mount-of-moon-1-1024x606.jpg",
         "fate-line-starts-from-mount-of-moon-1-1024x606.jpg", "start in moon zone x<0.35 y>0.65 (512)",
         "foreign/women help, trading/service", FatePriority::P0, true},
        {"joined_life_start", FateFigureGroup::Origin, "#Fate_line_and_life_line_joined",
         "https://palmistindian.com/wp-content/uploads/2020/10/fate-and-life-line-joint-at-start-1024x606.jpg",
         "fate-and-life-line-joint-at-start-1024x606.jpg", "start within 10px of life lower y>0.75",
         "self-made, own effort", FatePriority::P1, true},
        {"origin_from_head", FateFigureGroup::Origin, "#Fate_line_and_head_line_merged",
         "https://palmistindian.com/wp-content/uploads/2020/10/fate-line-originating-form-headline-1-1024x606.jpg",
         "fate-line-originating-form-headline-1-1024x606.jpg", "overlap>30px collinear col>0.85 with head",
         "mental idea changes life", FatePriority::P1, true},
        // ---- Group 2: Course ----
        {"short", FateFigureGroup::Course, "#Short_fate_line_its_meaning",
         "https://palmistindian.com/wp-content/uploads/2020/10/short-fate-line-1024x606.jpg",
         "short-fate-line-1024x606.jpg", "arc<0.4*palmH && tort<1.2", "high-pay possible if Mercury prominent",
         FatePriority::P0, true},
        {"long", FateFigureGroup::Course, "#A_long_fate_line",
         "https://palmistindian.com/wp-content/uploads/2020/10/long-fate-line-1024x606.jpg",
         "long-fate-line-1024x606.jpg", "len>0.70*palmH && conf>0.75", "stability, not more money", FatePriority::P0,
         true},
        {"broken", FateFigureGroup::Course, "#Broken_fate_line_palmistry",
         "https://palmistindian.com/wp-content/uploads/2020/10/fate-line-broken-1024x606.jpg",
         "fate-line-broken-1024x606.jpg", "gapCount>=1 or discontinuity>45px (CreaseLinkOptions::max_gap_distance)",
         "temporary instability, age from y", FatePriority::P0, true},
        {"double_parallel", FateFigureGroup::Course, "#Double_Fate_line_parallel_fate_line_in_palmistry",
         "https://palmistindian.com/wp-content/uploads/2020/10/double-fate-line-1024x606.jpg",
         "double-fate-line-1024x606.jpg", "2 parallel chains dist 8-25px overlap y>0.5", "dual income, extra pillar",
         FatePriority::P1, true},
        {"y_shaped", FateFigureGroup::Course, "#Y_shaped_Fate_line",
         "https://palmistindian.com/wp-content/uploads/2020/11/fate-line-Y-shaped-1024x606.jpg",
         "fate-line-Y-shaped-1024x606.jpg", "fork top angle 25-60deg near y<0.25 Saturn",
         "accumulation good (net catches energy)", FatePriority::P1, true},
        {"forked_up", FateFigureGroup::Course, "#A_forked_fate_line",
         "https://palmistindian.com/wp-content/uploads/2020/10/forked-fate-line-1024x606.jpg",
         "forked-fate-line-1024x606.jpg", "trident 3 tips toward saturn y<0.25", "excellent", FatePriority::P1, true},
        {"branches_up", FateFigureGroup::Course, "#Fate_line_branches_end",
         "https://palmistindian.com/wp-content/uploads/2020/10/fate-line-branches-towards-fingers-1024x606.jpg",
         "fate-line-branches-towards-fingers-1024x606.jpg", "end fork toward saturn angle<30deg", "good hard work",
         FatePriority::P1, true},
        {"toward_mars_negative", FateFigureGroup::Course, "#Fate_line_with_branches_towards_Mars_negative",
         "https://palmistindian.com/wp-content/uploads/2020/10/fate-line-branches-towards-mars-negative-1024x606.jpg",
         "fate-line-branches-towards-mars-negative-1024x606.jpg", "branch vector to mars_negative x<0.25 y0.40-0.55",
         "loss flag (money toward fire)", FatePriority::P1, true},
        {"parallel_life", FateFigureGroup::Course, "#Fate_line_and_life_line_parallel",
         "https://palmistindian.com/wp-content/uploads/2020/10/fate-line-and-life-line-run-parallel-1-1024x606.jpg",
         "fate-line-and-life-line-run-parallel-1-1024x606.jpg", "parallel dist 5-15px overlap y0.75-0.45",
         "own effort till 32", FatePriority::P1, true},
        // ---- Group 3: Termination / interaction ----
        {"stops_at_heart_53", FateFigureGroup::Termination, "#Fate_line_stops_at_heart_line",
         "https://palmistindian.com/wp-content/uploads/2020/10/fate-line-ends-on-heart-line-1024x606.jpg",
         "fate-line-ends-on-heart-line-1024x606.jpg", "intersect fate x heart (ZoneIntersector)",
         "negatable by strong Venus/Mercury/Sun", FatePriority::P0, true},
        {"stops_at_head_35", FateFigureGroup::Termination, "#Fate_line_stops_at_head_line",
         "https://palmistindian.com/wp-content/uploads/2020/10/fate-line-ends-on-head-line-1024x606.jpg",
         "fate-line-ends-on-head-line-1024x606.jpg", "intersect fate x head", "trouble 35 (career/family load)",
         FatePriority::P0, true},
        {"merged_head", FateFigureGroup::Termination, "#Fate_line_and_head_line_merged",
         "https://palmistindian.com/wp-content/uploads/2020/10/fate-line-and-headline-merged-at-start-1024x606.jpg",
         "fate-line-and-headline-merged-at-start-1024x606.jpg", "overlap>30px collinear with head",
         "gain via mental aptitude", FatePriority::P1, true},
        {"cross_sun", FateFigureGroup::Termination, "#Fate_line_and_sun_line_cross",
         "https://palmistindian.com/wp-content/uploads/2020/10/fate-and-sun-line-cross-each-other-1024x606.jpg",
         "fate-and-sun-line-cross-each-other-1024x606.jpg", "intersect fate x sun(apollo), both firm",
         "neutral if both deep/clear", FatePriority::P1, false},
        {"cross_life_to_mars", FateFigureGroup::Termination, "#Fate_line_and_life_line_cross",
         "https://palmistindian.com/wp-content/uploads/2020/10/fate-line-crosses-the-life-line-1024x606.jpg",
         "fate-line-crosses-the-life-line-1024x606.jpg", "fate crosses life toward mars_negative",
         "fire loss (also fate-line-moves-towards-mars-negative-1024x606.jpg)", FatePriority::P1, true},
        // ---- Group 4: Marks ----
        {"island_fish", FateFigureGroup::Marks, "#Fish_sign_on_Fate_line",
         "https://palmistindian.com/wp-content/uploads/2020/11/fate-line-fish-sign-1024x606.jpg",
         "fate-line-fish-sign-1024x606.jpg", "closed loop + 2 tails; author: bad not lucky",
         "setback (also isalnd-on-fate-line-1024x606.jpg loss)", FatePriority::P2, false},
        {"triangle", FateFigureGroup::Marks, "#Triangle_on_fate_line_in_palmistry",
         "https://palmistindian.com/wp-content/uploads/2020/10/triangle-on-fate-line-1024x606.jpg",
         "triangle-on-fate-line-1024x606.jpg",
         "3 segs closed triangle area>30 (header triangle-on-fate-line-702x526.jpg)", "property purchase luck",
         FatePriority::P2, false},
        {"cross", FateFigureGroup::Marks, "#Cross_on_fate_line_in_palmistry",
         "https://palmistindian.com/wp-content/uploads/2020/10/cross-on-fate-line-1024x606.jpg",
         "cross-on-fate-line-1024x606.jpg", "X 2 segs angle 70-110deg (also cross-on-fate-line-1-1024x606.jpg)",
         "job/business loss, strength ~ tort*len", FatePriority::P2, false},
        {"square", FateFigureGroup::Marks, "#Square_on_fate_line_in_palmistry",
         "https://palmistindian.com/wp-content/uploads/2020/10/square-on-fate-line-1-1024x606.jpg",
         "square-on-fate-line-1-1024x606.jpg", "4 segs rectangle area>40 enclosing gap", "preservation from loss",
         FatePriority::P2, false},
        {"bad_saturn_mount", FateFigureGroup::Marks, "#Fate_line_and_mount_of_Saturn",
         "https://palmistindian.com/wp-content/uploads/2020/10/fate-line-good-but-bad-saturn-mount-1-1024x606.jpg",
         "fate-line-good-but-bad-saturn-mount-1-1024x606.jpg", "grill/cross/star on saturn_zone (Hough)",
         "hardship even if fate good", FatePriority::P2, false},
        {"bad_mercury_mount", FateFigureGroup::Marks, "#Fate_line_and_mount_of_Mercury",
         "https://palmistindian.com/wp-content/uploads/2020/10/fate-line-good-but-bad-mercury-1024x606.jpg",
         "fate-line-good-but-bad-mercury-1024x606.jpg", "grill/island/horiz on mercury_zone", "poor decision flag",
         FatePriority::P2, false},
        // ---- Group 5: Age ----
        {"age_map", FateFigureGroup::Age, "#Fate_line_age_in_palmistry",
         "https://palmistindian.com/wp-content/uploads/2020/10/age-on-fate-line-1024x606.jpg",
         "age-on-fate-line-1024x606.jpg", "y->age: heart 53, head 35, pre-head 33, 28", "report ageAt(y)",
         FatePriority::P0, true},
        // ---- Group 6: Context (no new geometry) ----
        {"weight_15pct", FateFigureGroup::Context, "#Too_much_importance_is_given_to_fate_line",
         "https://palmistindian.com/wp-content/uploads/2020/10/fate-line-too-much-of-importance-1024x606.jpg",
         "fate-line-too-much-of-importance-1024x606.jpg", "weight fate 0.15 in overall",
         "down-weight in VlmPromptFormatter", FatePriority::P1, false},
        {"money_role", FateFigureGroup::Context,
         "#What_role_does_fate_line_play_in_determining_how_much_money_we_will_have_in_our_lives",
         "https://palmistindian.com/wp-content/uploads/2020/10/fate-and-other-line-importance-1024x606.jpg",
         "fate-and-other-line-importance-1024x606.jpg", "require fate && mercury/saturn/sun fusion",
         "downgrade if mounts grilled", FatePriority::P1, false},
        {"luck_career_alias", FateFigureGroup::Context, "#Luck_line_in_palmistry",
         "https://palmistindian.com/wp-content/uploads/2020/11/fate-line-luck-line-1024x606.jpg",
         "fate-line-luck-line-1024x606.jpg", "alias luck==fate (also career-line-1024x606.jpg)",
         "career_stable if long", FatePriority::P1, false},
        {"govt_job", FateFigureGroup::Context, "#Government_job_in_hand",
         "https://palmistindian.com/wp-content/uploads/2020/11/fate-line-and-government-job-1024x606.jpg",
         "fate-line-and-government-job-1024x606.jpg", "needs sun+jupiter+mercury triple, not fate alone",
         "future GovernmentJobPack", FatePriority::P3, false},
    };
}

// Future tick list: flip `implemented` per case as detectors land.
// P0 first (present/stops/broken/age), then P1 branches, then P2 shapes.

} // namespace CurvEngine::domain
