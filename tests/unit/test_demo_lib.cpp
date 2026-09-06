#include <gtest/gtest.h>

#include "curv/tools/demo_lib.hpp"

namespace {

TEST(DemoLibTest, DemoRunSucceeds) {
    EXPECT_EQ(CurvEngine::tools::runDesktopDemo(), 0);
}

} // namespace
