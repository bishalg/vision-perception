#include <gtest/gtest.h>

#include "curv/domain/LineLengthThresholds.hpp"

using namespace CurvEngine::domain;

TEST(LineLengthTableTest, DefaultsCoverAllFourLinesUncalibrated) {
    const auto table = LineLengthTable::defaults();
    ASSERT_EQ(table.size(), 4u);
    for (const char* id : {"heart_line", "head_line", "life_line", "fate_line"}) {
        const auto* row = LineLengthTable::find(table, id);
        ASSERT_NE(row, nullptr) << id;
        EXPECT_FALSE(row->calibrated) << id << " must ship uncalibrated";
        EXPECT_FALSE(row->classical_note.empty()) << id;
    }
    EXPECT_EQ(LineLengthTable::find(table, "nope"), nullptr);
}

TEST(LineLengthTableTest, VerticalLinesNormalizeAgainstHeight) {
    const auto table = LineLengthTable::defaults();
    const auto* fate = LineLengthTable::find(table, "fate_line");
    const auto* life = LineLengthTable::find(table, "life_line");
    ASSERT_NE(fate, nullptr);
    ASSERT_NE(life, nullptr);
    EXPECT_EQ(fate->basis, "palm_height");
    EXPECT_EQ(life->basis, "palm_height");
    // Fate table agrees with the FateLineAnalyzer precedent (0.40/0.70).
    EXPECT_FLOAT_EQ(fate->short_ratio, 0.40F);
    EXPECT_FLOAT_EQ(fate->long_ratio, 0.70F);
}

TEST(LineLengthTableTest, HorizontalLinesNormalizeAgainstWidth) {
    const auto table = LineLengthTable::defaults();
    const auto* heart = LineLengthTable::find(table, "heart_line");
    const auto* head = LineLengthTable::find(table, "head_line");
    ASSERT_NE(heart, nullptr);
    ASSERT_NE(head, nullptr);
    EXPECT_EQ(heart->basis, "palm_width");
    EXPECT_EQ(head->basis, "palm_width");
}

TEST(LineLengthTableTest, ClassifyShortMediumLong) {
    const auto table = LineLengthTable::defaults();
    const auto* fate = LineLengthTable::find(table, "fate_line");
    ASSERT_NE(fate, nullptr);
    // 512-canonical patch: 0.40*512=204.8 short boundary, 0.70*512=358.4 long.
    const auto short_cls = LineLengthTable::classify(*fate, 150.0F, 512.0F);
    EXPECT_TRUE(short_cls.is_short);
    EXPECT_FALSE(short_cls.is_long);
    const auto long_cls = LineLengthTable::classify(*fate, 400.0F, 512.0F);
    EXPECT_FALSE(long_cls.is_short);
    EXPECT_TRUE(long_cls.is_long);
    const auto med_cls = LineLengthTable::classify(*fate, 280.0F, 512.0F);
    EXPECT_FALSE(med_cls.is_short);
    EXPECT_FALSE(med_cls.is_long);
}

TEST(LineLengthTableTest, DegenerateBasisIsSafe) {
    const auto table = LineLengthTable::defaults();
    const auto* heart = LineLengthTable::find(table, "heart_line");
    ASSERT_NE(heart, nullptr);
    const auto zero = LineLengthTable::classify(*heart, 100.0F, 0.0F);
    EXPECT_FALSE(zero.is_short);
    EXPECT_FALSE(zero.is_long);
}
