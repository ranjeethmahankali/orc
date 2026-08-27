extern "C"
{
#include <third_party/unity.h>
}

#include "orc_sdk/deck.hpp"
#include "orc_sdk/error.hpp"

#include <sstream>
#include <vector>

using orc_sdk::Deck;

static std::vector<uint8_t> mark_depths(Deck<int> const &d)
{
  std::vector<uint8_t> out;
  for (auto const &m : d.marks())
    out.push_back(m.depth);
  return out;
}

static std::vector<uint64_t> mark_positions(Deck<int> const &d)
{
  std::vector<uint64_t> out;
  for (auto const &m : d.marks())
    out.push_back(m.pos);
  return out;
}

static void test_build_flat()
{
  auto d = Deck<int>::build({0, 1, 2, 3});
  TEST_ASSERT_EQUAL_UINT64(4, d.len());
  TEST_ASSERT_EQUAL_UINT8(1, d.max_depth());
  TEST_ASSERT_EQUAL_INT(0, d[0]);
  TEST_ASSERT_EQUAL_INT(1, d[1]);
  TEST_ASSERT_EQUAL_INT(2, d[2]);
  TEST_ASSERT_EQUAL_INT(3, d[3]);
  auto depths = mark_depths(d);
  auto pos    = mark_positions(d);
  TEST_ASSERT_EQUAL_UINT64(1, depths.size());
  TEST_ASSERT_EQUAL_UINT8(0, depths[0]);
  TEST_ASSERT_EQUAL_UINT64(1, pos.size());
  TEST_ASSERT_EQUAL_UINT64(0, pos[0]);
}

static void test_build_depth2()
{
  auto d = Deck<int>::build({{0, 1}, {2, 3}});
  TEST_ASSERT_EQUAL_UINT64(4, d.len());
  TEST_ASSERT_EQUAL_UINT8(2, d.max_depth());
  TEST_ASSERT_EQUAL_INT(0, d[0]);
  TEST_ASSERT_EQUAL_INT(1, d[1]);
  TEST_ASSERT_EQUAL_INT(2, d[2]);
  TEST_ASSERT_EQUAL_INT(3, d[3]);
  auto depths = mark_depths(d);
  auto pos    = mark_positions(d);
  TEST_ASSERT_EQUAL_UINT64(2, depths.size());
  TEST_ASSERT_EQUAL_UINT8(1, depths[0]);
  TEST_ASSERT_EQUAL_UINT8(0, depths[1]);
  TEST_ASSERT_EQUAL_UINT64(2, pos.size());
  TEST_ASSERT_EQUAL_UINT64(0, pos[0]);
  TEST_ASSERT_EQUAL_UINT64(2, pos[1]);
}

static void test_build_depth3()
{
  auto d = Deck<int>::build({{{0, 1}, {2, 3}}, {{4, 5}, {6, 7}}});
  TEST_ASSERT_EQUAL_UINT64(8, d.len());
  TEST_ASSERT_EQUAL_UINT8(3, d.max_depth());
  for (int i = 0; i < 8; ++i)
    TEST_ASSERT_EQUAL_INT(i, d[static_cast<size_t>(i)]);
  auto depths = mark_depths(d);
  auto pos    = mark_positions(d);
  TEST_ASSERT_EQUAL_UINT64(4, depths.size());
  TEST_ASSERT_EQUAL_UINT8(2, depths[0]);
  TEST_ASSERT_EQUAL_UINT8(0, depths[1]);
  TEST_ASSERT_EQUAL_UINT8(1, depths[2]);
  TEST_ASSERT_EQUAL_UINT8(0, depths[3]);
  TEST_ASSERT_EQUAL_UINT64(0, pos[0]);
  TEST_ASSERT_EQUAL_UINT64(2, pos[1]);
  TEST_ASSERT_EQUAL_UINT64(4, pos[2]);
  TEST_ASSERT_EQUAL_UINT64(6, pos[3]);
}

static void test_build_depth3_3way()
{
  auto d = Deck<int>::build({{{0, 1}, {2, 3}}, {{4, 5}, {6, 7}}, {{8, 9}, {10, 11}}});
  TEST_ASSERT_EQUAL_UINT64(12, d.len());
  TEST_ASSERT_EQUAL_UINT8(3, d.max_depth());
  auto depths = mark_depths(d);
  auto pos    = mark_positions(d);
  TEST_ASSERT_EQUAL_UINT64(6, depths.size());
  uint8_t  expected_depths[] = {2, 0, 1, 0, 1, 0};
  uint64_t expected_pos[]    = {0, 2, 4, 6, 8, 10};
  for (size_t i = 0; i < 6; ++i) {
    TEST_ASSERT_EQUAL_UINT8(expected_depths[i], depths[i]);
    TEST_ASSERT_EQUAL_UINT64(expected_pos[i], pos[i]);
  }
}

static void test_build_single()
{
  auto d = Deck<int>::build({42});
  TEST_ASSERT_EQUAL_UINT64(1, d.len());
  TEST_ASSERT_EQUAL_UINT8(1, d.max_depth());
  TEST_ASSERT_EQUAL_INT(42, d[0]);
  auto depths = mark_depths(d);
  auto pos    = mark_positions(d);
  TEST_ASSERT_EQUAL_UINT64(1, depths.size());
  TEST_ASSERT_EQUAL_UINT8(0, depths[0]);
  TEST_ASSERT_EQUAL_UINT64(0, pos[0]);
}

static void test_build_empty_groups()
{
  // Depth 2 with an empty first group.
  auto d = Deck<int>::build({{}, {1, 2}});
  TEST_ASSERT_EQUAL_UINT64(2, d.len());
  TEST_ASSERT_EQUAL_UINT8(2, d.max_depth());
  TEST_ASSERT_EQUAL_INT(1, d[0]);
  TEST_ASSERT_EQUAL_INT(2, d[1]);
}

static void test_build_ragged()
{
  // Groups of different sizes.
  auto d = Deck<int>::build({{1}, {2, 3, 4}, {5, 6}});
  TEST_ASSERT_EQUAL_UINT64(6, d.len());
  TEST_ASSERT_EQUAL_UINT8(2, d.max_depth());
  auto pos = mark_positions(d);
  TEST_ASSERT_EQUAL_UINT64(3, pos.size());
  TEST_ASSERT_EQUAL_UINT64(0, pos[0]);
  TEST_ASSERT_EQUAL_UINT64(1, pos[1]);
  TEST_ASSERT_EQUAL_UINT64(4, pos[2]);
}

static void test_flatten()
{
  auto d = Deck<int>::build({{1, 2, 3}, {4, 5, 6}, {7, 8, 9}});
  d.flatten();
  auto expected = Deck<int>::build({1, 2, 3, 4, 5, 6, 7, 8, 9});
  TEST_ASSERT_TRUE(d == expected);
}

static void test_graft()
{
  auto d = Deck<int>::build({{1, 2, 3}, {4, 5, 6}, {7, 8, 9}});
  d.graft();
  // Verify via manual push — single-element inner lists are ambiguous for build().
  Deck<int> expected;
  int       vals[] = {1, 2, 3, 4, 5, 6, 7, 8, 9};
  for (int i = 0; i < 9; ++i) {
    uint8_t depth = (i % 3 == 0) ? (i == 0 ? 3 : 2) : 1;
    expected.push(vals[i], depth);
  }
  TEST_ASSERT_TRUE(d == expected);
}

static void test_simplify()
{
  auto d = Deck<int>::build({{{1, 2, 3}}, {{4, 5, 6}}, {{7, 8, 9}}});
  d.simplify();
  auto expected = Deck<int>::build({{1, 2, 3}, {4, 5, 6}, {7, 8, 9}});
  TEST_ASSERT_TRUE(d == expected);
}

static void test_clear()
{
  auto d = Deck<int>::build({{1, 2}, {3, 4}});
  d.clear();
  TEST_ASSERT_EQUAL_UINT64(0, d.len());
  TEST_ASSERT_TRUE(d.is_empty());
  TEST_ASSERT_EQUAL_UINT8(0, d.max_depth());
}

static void test_push_manual()
{
  // Build {{1, 2}, {3, 4}} manually via push.
  Deck<int> d;
  d.push(1, 2);
  d.push(2, 0);
  d.push(3, 1);
  d.push(4, 0);
  auto expected = Deck<int>::build({{1, 2}, {3, 4}});
  TEST_ASSERT_TRUE(d == expected);
}

void setUp(void) {}
void tearDown(void) {}

int main(void)
{
  UNITY_BEGIN();
  RUN_TEST(test_build_flat);
  RUN_TEST(test_build_depth2);
  RUN_TEST(test_build_depth3);
  RUN_TEST(test_build_depth3_3way);
  RUN_TEST(test_build_single);
  RUN_TEST(test_build_empty_groups);
  RUN_TEST(test_build_ragged);
  RUN_TEST(test_flatten);
  RUN_TEST(test_graft);
  RUN_TEST(test_simplify);
  RUN_TEST(test_clear);
  RUN_TEST(test_push_manual);
  return UNITY_END();
}
