extern "C"
{
#include <third_party/unity.h>
}

#include "orc_sdk/deck.hpp"
#include "orc_sdk/error.hpp"

#include <sstream>
#include <string>
#include <vector>

using orc_sdk::Deck;
using orc_sdk::DeckView;
using orc_sdk::DeckWriter;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

static std::vector<uint8_t> mark_depths(Deck<int> const &d)
{
  std::vector<uint8_t> out;
  for (OrcMark const &m : d.marks())
    out.push_back(m.depth);
  return out;
}

static std::vector<uint64_t> mark_positions(Deck<int> const &d)
{
  std::vector<uint64_t> out;
  for (OrcMark const &m : d.marks())
    out.push_back(m.pos);
  return out;
}

static Deck<size_t> binary_deck(uint8_t depth)
{
  Deck<size_t> d;
  size_t count = static_cast<size_t>(1) << depth;
  for (size_t i = 0; i < count; ++i) {
    uint8_t tz = 0;
    if (i > 0) {
      size_t v = i;
      while ((v & 1) == 0) { ++tz; v >>= 1; }
    } else {
      tz = depth;
    }
    d.push(i, std::min(tz, depth));
  }
  return d;
}

// Collect depth-1 groups from a depth-2 deck.
static std::vector<std::vector<uint32_t>> tree2(Deck<uint32_t> const &d)
{
  std::vector<std::vector<uint32_t>> result;
  DeckView<uint32_t> v2 = d.view(2);
  if (v2.empty()) {
    do {
      DeckView<uint32_t> v1 = v2.child();
      if (v1.empty()) {
        do {
          std::span<uint32_t const> s = v1.as_slice();
          result.push_back(std::vector<uint32_t>(s.begin(), s.end()));
        } while (v1.advance());
      }
    } while (v2.advance());
  }
  return result;
}

// Collect depth-1 groups from a depth-3 deck, grouped by depth-2.
static std::vector<std::vector<std::vector<uint32_t>>> tree3(Deck<uint32_t> const &d)
{
  std::vector<std::vector<std::vector<uint32_t>>> result;
  DeckView<uint32_t> v3 = d.view(3);
  if (v3.empty()) {
    do {
      DeckView<uint32_t> v2 = v3.child();
      if (v2.empty()) {
        do {
          std::vector<std::vector<uint32_t>> mid_group;
          DeckView<uint32_t> v1 = v2.child();
          if (v1.empty()) {
            do {
              std::span<uint32_t const> s = v1.as_slice();
              mid_group.push_back(std::vector<uint32_t>(s.begin(), s.end()));
            } while (v1.advance());
          }
          result.push_back(std::move(mid_group));
        } while (v2.advance());
      }
    } while (v3.advance());
  }
  return result;
}

static std::string to_str(Deck<int> const &d)
{
  std::ostringstream ss;
  ss << d;
  return ss.str();
}

// ---------------------------------------------------------------------------
// Deck build tests
// ---------------------------------------------------------------------------

static void test_build_flat()
{
  Deck<int> d = Deck<int>::build({0, 1, 2, 3});
  TEST_ASSERT_EQUAL_UINT64(4, d.size());
  TEST_ASSERT_EQUAL_UINT8(1, d.max_depth());
  TEST_ASSERT_EQUAL_INT(0, d[0]);
  TEST_ASSERT_EQUAL_INT(1, d[1]);
  TEST_ASSERT_EQUAL_INT(2, d[2]);
  TEST_ASSERT_EQUAL_INT(3, d[3]);
  std::vector<uint8_t>  depths = mark_depths(d);
  std::vector<uint64_t> pos    = mark_positions(d);
  TEST_ASSERT_EQUAL_UINT64(1, depths.size());
  TEST_ASSERT_EQUAL_UINT8(0, depths[0]);
  TEST_ASSERT_EQUAL_UINT64(1, pos.size());
  TEST_ASSERT_EQUAL_UINT64(0, pos[0]);
}

static void test_build_depth2()
{
  Deck<int> d = Deck<int>::build<2>({{0, 1}, {2, 3}});
  TEST_ASSERT_EQUAL_UINT64(4, d.size());
  TEST_ASSERT_EQUAL_UINT8(2, d.max_depth());
  TEST_ASSERT_EQUAL_INT(0, d[0]);
  TEST_ASSERT_EQUAL_INT(1, d[1]);
  TEST_ASSERT_EQUAL_INT(2, d[2]);
  TEST_ASSERT_EQUAL_INT(3, d[3]);
  std::vector<uint8_t>  depths = mark_depths(d);
  std::vector<uint64_t> pos    = mark_positions(d);
  TEST_ASSERT_EQUAL_UINT64(2, depths.size());
  TEST_ASSERT_EQUAL_UINT8(1, depths[0]);
  TEST_ASSERT_EQUAL_UINT8(0, depths[1]);
  TEST_ASSERT_EQUAL_UINT64(2, pos.size());
  TEST_ASSERT_EQUAL_UINT64(0, pos[0]);
  TEST_ASSERT_EQUAL_UINT64(2, pos[1]);
}

static void test_build_depth3()
{
  Deck<int> d = Deck<int>::build<3>({{{0, 1}, {2, 3}}, {{4, 5}, {6, 7}}});
  TEST_ASSERT_EQUAL_UINT64(8, d.size());
  TEST_ASSERT_EQUAL_UINT8(3, d.max_depth());
  for (int i = 0; i < 8; ++i)
    TEST_ASSERT_EQUAL_INT(i, d[static_cast<size_t>(i)]);
  std::vector<uint8_t>  depths = mark_depths(d);
  std::vector<uint64_t> pos    = mark_positions(d);
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
  Deck<int> d = Deck<int>::build<3>({{{0, 1}, {2, 3}}, {{4, 5}, {6, 7}}, {{8, 9}, {10, 11}}});
  TEST_ASSERT_EQUAL_UINT64(12, d.size());
  TEST_ASSERT_EQUAL_UINT8(3, d.max_depth());
  std::vector<uint8_t>  depths = mark_depths(d);
  std::vector<uint64_t> pos    = mark_positions(d);
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
  Deck<int> d = Deck<int>::build({42});
  TEST_ASSERT_EQUAL_UINT64(1, d.size());
  TEST_ASSERT_EQUAL_UINT8(1, d.max_depth());
  TEST_ASSERT_EQUAL_INT(42, d[0]);
  std::vector<uint8_t>  depths = mark_depths(d);
  std::vector<uint64_t> pos    = mark_positions(d);
  TEST_ASSERT_EQUAL_UINT64(1, depths.size());
  TEST_ASSERT_EQUAL_UINT8(0, depths[0]);
  TEST_ASSERT_EQUAL_UINT64(0, pos[0]);
}

static void test_build_empty_groups()
{
  Deck<int> d = Deck<int>::build<2>({{}, {1, 2}});
  TEST_ASSERT_EQUAL_UINT64(2, d.size());
  TEST_ASSERT_EQUAL_UINT8(2, d.max_depth());
  TEST_ASSERT_EQUAL_INT(1, d[0]);
  TEST_ASSERT_EQUAL_INT(2, d[1]);
}

static void test_build_ragged()
{
  Deck<int> d = Deck<int>::build<2>({{1}, {2, 3, 4}, {5, 6}});
  TEST_ASSERT_EQUAL_UINT64(6, d.size());
  TEST_ASSERT_EQUAL_UINT8(2, d.max_depth());
  std::vector<uint64_t> pos = mark_positions(d);
  TEST_ASSERT_EQUAL_UINT64(3, pos.size());
  TEST_ASSERT_EQUAL_UINT64(0, pos[0]);
  TEST_ASSERT_EQUAL_UINT64(1, pos[1]);
  TEST_ASSERT_EQUAL_UINT64(4, pos[2]);
}

// ---------------------------------------------------------------------------
// Deck mutation tests
// ---------------------------------------------------------------------------

static void test_flatten()
{
  Deck<int> d = Deck<int>::build<2>({{1, 2, 3}, {4, 5, 6}, {7, 8, 9}});
  d.flatten();
  Deck<int> expected = Deck<int>::build({1, 2, 3, 4, 5, 6, 7, 8, 9});
  TEST_ASSERT_TRUE(d == expected);
}

static void test_graft()
{
  Deck<int> d = Deck<int>::build<2>({{1, 2, 3}, {4, 5, 6}, {7, 8, 9}});
  d.graft();
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
  Deck<int> d        = Deck<int>::build<3>({{{1, 2, 3}}, {{4, 5, 6}}, {{7, 8, 9}}});
  Deck<int> expected = Deck<int>::build<2>({{1, 2, 3}, {4, 5, 6}, {7, 8, 9}});
  d.simplify();
  TEST_ASSERT_TRUE(d == expected);
}

static void test_clear()
{
  Deck<int> d = Deck<int>::build<2>({{1, 2}, {3, 4}});
  d.clear();
  TEST_ASSERT_EQUAL_UINT64(0, d.size());
  TEST_ASSERT_TRUE(d.empty());
  TEST_ASSERT_EQUAL_UINT8(0, d.max_depth());
}

static void test_push_manual()
{
  Deck<int> d;
  d.push(1, 2);
  d.push(2, 0);
  d.push(3, 1);
  d.push(4, 0);
  Deck<int> expected = Deck<int>::build<2>({{1, 2}, {3, 4}});
  TEST_ASSERT_TRUE(d == expected);
}

// ---------------------------------------------------------------------------
// Display tests
// ---------------------------------------------------------------------------

static void test_display_empty()
{
  Deck<int> d;
  TEST_ASSERT_EQUAL_STRING("<empty_deck>\n", to_str(d).c_str());
}

static void test_display_flat()
{
  Deck<int>   d = Deck<int>::build({1, 2, 3});
  char const *expected =
    "  1 ---| 1\n"
    "       | 2\n"
    "       | 3\n";
  TEST_ASSERT_EQUAL_STRING(expected, to_str(d).c_str());
}

static void test_display_depth2()
{
  Deck<int>   d = Deck<int>::build<2>({{1, 2}, {3, 4}});
  char const *expected =
    "  2 ------| 1\n"
    "          | 2\n"
    "     1 ---| 3\n"
    "          | 4\n";
  TEST_ASSERT_EQUAL_STRING(expected, to_str(d).c_str());
}

static void test_display_depth3()
{
  Deck<int>   d = Deck<int>::build<3>({{{10, 20}, {30, 40}}, {{50, 60}, {70, 80}}});
  char const *expected =
    "  3 ---------| 10\n"
    "             | 20\n"
    "        1 ---| 30\n"
    "             | 40\n"
    "     2 ------| 50\n"
    "             | 60\n"
    "        1 ---| 70\n"
    "             | 80\n";
  TEST_ASSERT_EQUAL_STRING(expected, to_str(d).c_str());
}

static void test_display_single()
{
  Deck<int> d = Deck<int>::build({42});
  TEST_ASSERT_EQUAL_STRING("  1 ---| 42\n", to_str(d).c_str());
}

static void test_display_empty_group()
{
  Deck<int>   d = Deck<int>::build<2>({{}, {1, 2}});
  char const *expected =
    "  2 ------|\n"
    "     1 ---| 1\n"
    "          | 2\n";
  TEST_ASSERT_EQUAL_STRING(expected, to_str(d).c_str());
}

// ---------------------------------------------------------------------------
// DeckView: basic iteration (t_basic_ops)
// ---------------------------------------------------------------------------

static void test_view_basic_depth5()
{
  Deck<size_t> d = binary_deck(5);
  TEST_ASSERT_EQUAL_UINT64(32, d.size());
  TEST_ASSERT_EQUAL_UINT8(5, d.max_depth());
  size_t counter = 0;
  DeckView<size_t> v5 = d.view(5);
  do {
    TEST_ASSERT_EQUAL_UINT8(5, v5.depth());
    TEST_ASSERT_EQUAL_UINT64(32, v5.size());
    DeckView<size_t> v4 = v5.child();
    do {
      TEST_ASSERT_EQUAL_UINT8(4, v4.depth());
      TEST_ASSERT_EQUAL_UINT64(16, v4.size());
      DeckView<size_t> v3 = v4.child();
      do {
        TEST_ASSERT_EQUAL_UINT8(3, v3.depth());
        TEST_ASSERT_EQUAL_UINT64(8, v3.size());
        DeckView<size_t> v2 = v3.child();
        do {
          TEST_ASSERT_EQUAL_UINT8(2, v2.depth());
          TEST_ASSERT_EQUAL_UINT64(4, v2.size());
          DeckView<size_t> v1 = v2.child();
          do {
            TEST_ASSERT_EQUAL_UINT8(1, v1.depth());
            DeckView<size_t> v0 = v1.child();
            do {
              TEST_ASSERT_EQUAL_UINT8(0, v0.depth());
              TEST_ASSERT_EQUAL_UINT64(1, v0.size());
              TEST_ASSERT_EQUAL_UINT64(counter, v0.as_ref());
              ++counter;
            } while (v0.advance());
          } while (v1.advance());
        } while (v2.advance());
      } while (v3.advance());
    } while (v4.advance());
  } while (v5.advance());
  TEST_ASSERT_EQUAL_UINT64(32, counter);
}

static void test_view_from_level4()
{
  Deck<size_t> d = binary_deck(5);
  size_t counter = 0;
  DeckView<size_t> v4 = d.view(4);
  do {
    TEST_ASSERT_EQUAL_UINT8(4, v4.depth());
    DeckView<size_t> v3 = v4.child();
    do {
      DeckView<size_t> v2 = v3.child();
      do {
        DeckView<size_t> v1 = v2.child();
        do {
          DeckView<size_t> v0 = v1.child();
          do {
            TEST_ASSERT_EQUAL_UINT64(counter, v0.as_ref());
            ++counter;
          } while (v0.advance());
        } while (v1.advance());
      } while (v2.advance());
    } while (v3.advance());
  } while (v4.advance());
  TEST_ASSERT_EQUAL_UINT64(32, counter);
}

static void test_view_from_level3()
{
  Deck<size_t> d = binary_deck(5);
  size_t counter = 0;
  DeckView<size_t> v3 = d.view(3);
  do {
    TEST_ASSERT_EQUAL_UINT8(3, v3.depth());
    DeckView<size_t> v2 = v3.child();
    do {
      DeckView<size_t> v1 = v2.child();
      do {
        DeckView<size_t> v0 = v1.child();
        do {
          TEST_ASSERT_EQUAL_UINT64(counter, v0.as_ref());
          ++counter;
        } while (v0.advance());
      } while (v1.advance());
    } while (v2.advance());
  } while (v3.advance());
  TEST_ASSERT_EQUAL_UINT64(32, counter);
}

// ---------------------------------------------------------------------------
// DeckView: edge cases (t_deck_edge_cases)
// ---------------------------------------------------------------------------

static void test_view_edge_cases()
{
  // Empty deck: view(0) has nothing.
  {
    Deck<size_t> empty;
    DeckView<size_t> v = empty.view(0);
    TEST_ASSERT_FALSE(v.empty());
  }
  // Single element at depth 0 (bare leaf).
  {
    Deck<size_t> d;
    d.push(42, 0);
    TEST_ASSERT_EQUAL_UINT64(1, d.size());
    TEST_ASSERT_EQUAL_UINT8(0, d.max_depth());
    TEST_ASSERT_TRUE(d.marks().empty());
    DeckView<size_t> v0 = d.view(0);
    TEST_ASSERT_TRUE(v0.empty());
    TEST_ASSERT_EQUAL_UINT64(42, v0.as_ref());
    TEST_ASSERT_FALSE(v0.advance());
  }
  // Single element at depth 1 (one-element list).
  {
    Deck<size_t> d;
    d.push(7, 1);
    TEST_ASSERT_EQUAL_UINT8(1, d.max_depth());
    // view(1) yields 1 group.
    size_t count = 0;
    DeckView<size_t> v1 = d.view(1);
    if (v1.empty()) { do { ++count; } while (v1.advance()); }
    TEST_ASSERT_EQUAL_UINT64(1, count);
    // That group has one item.
    DeckView<size_t> v = d.view(1);
    std::span<size_t const> s = v.as_slice();
    TEST_ASSERT_EQUAL_UINT64(1, s.size());
    TEST_ASSERT_EQUAL_UINT64(7, s[0]);
  }
  // clear() resets everything; re-use after clear.
  {
    Deck<size_t> d = binary_deck(3);
    d.clear();
    TEST_ASSERT_EQUAL_UINT64(0, d.size());
    TEST_ASSERT_EQUAL_UINT8(0, d.max_depth());
    d.push(1, 1);
    d.push(2, 0);
    TEST_ASSERT_EQUAL_UINT64(2, d.size());
    TEST_ASSERT_EQUAL_UINT8(1, d.max_depth());
    DeckView<size_t> v = d.view(1);
    std::span<size_t const> s = v.as_slice();
    TEST_ASSERT_EQUAL_UINT64(2, s.size());
    TEST_ASSERT_EQUAL_UINT64(1, s[0]);
    TEST_ASSERT_EQUAL_UINT64(2, s[1]);
  }
}

// ---------------------------------------------------------------------------
// DeckView: flatten with view verification (t_deck_flatten)
// ---------------------------------------------------------------------------

static void test_flatten_with_view()
{
  Deck<size_t> d = binary_deck(4);
  // Save items before.
  std::vector<size_t> items_before(d.items().begin(), d.items().end());
  d.flatten();
  // Items survived.
  std::span<size_t const> items_after = d.items();
  TEST_ASSERT_EQUAL_UINT64(items_before.size(), items_after.size());
  for (size_t i = 0; i < items_before.size(); ++i)
    TEST_ASSERT_EQUAL_UINT64(items_before[i], items_after[i]);
  TEST_ASSERT_EQUAL_UINT8(1, d.max_depth());
  TEST_ASSERT_EQUAL_UINT64(1, d.marks().size());
  TEST_ASSERT_EQUAL_UINT8(0, d.marks()[0].depth);
  TEST_ASSERT_EQUAL_UINT64(0, d.marks()[0].pos);
  // Iterating at depth 1 yields all elements as one flat group.
  size_t count = 0;
  DeckView<size_t> v1 = d.view(1);
  if (v1.empty()) { do { ++count; } while (v1.advance()); }
  TEST_ASSERT_EQUAL_UINT64(1, count);
  DeckView<size_t> v = d.view(1);
  std::span<size_t const> s = v.as_slice();
  TEST_ASSERT_EQUAL_UINT64(items_before.size(), s.size());
  for (size_t i = 0; i < items_before.size(); ++i)
    TEST_ASSERT_EQUAL_UINT64(items_before[i], s[i]);
  // Flatten single-element deck: no marks needed.
  Deck<size_t> d2;
  d2.push(5, 0);
  d2.flatten();
  TEST_ASSERT_EQUAL_UINT8(0, d2.max_depth());
  TEST_ASSERT_TRUE(d2.marks().empty());
  // Flatten is idempotent.
  Deck<size_t> d3 = binary_deck(3);
  d3.flatten();
  std::vector<uint8_t> depths_first;
  std::vector<uint64_t> pos_first;
  for (OrcMark const &m : d3.marks()) { depths_first.push_back(m.depth); pos_first.push_back(m.pos); }
  d3.flatten();
  std::vector<uint8_t> depths_second;
  std::vector<uint64_t> pos_second;
  for (OrcMark const &m : d3.marks()) { depths_second.push_back(m.depth); pos_second.push_back(m.pos); }
  TEST_ASSERT_TRUE(depths_first == depths_second);
  TEST_ASSERT_TRUE(pos_first == pos_second);
}

// ---------------------------------------------------------------------------
// DeckView: graft with view verification (t_deck_graft)
// ---------------------------------------------------------------------------

static void test_graft_with_view()
{
  Deck<size_t> d = binary_deck(3);
  std::vector<size_t> items_before(d.items().begin(), d.items().end());
  d.graft();
  TEST_ASSERT_EQUAL_UINT8(4, d.max_depth());
  // Items unchanged.
  std::span<size_t const> items_after = d.items();
  TEST_ASSERT_EQUAL_UINT64(items_before.size(), items_after.size());
  for (size_t i = 0; i < items_before.size(); ++i)
    TEST_ASSERT_EQUAL_UINT64(items_before[i], items_after[i]);
  // Iterate at depth 4 to verify structure.
  size_t counter = 0;
  DeckView<size_t> v4 = d.view(4);
  DeckView<size_t> v3c = v4.child();
  do {
    TEST_ASSERT_EQUAL_UINT8(3, v3c.depth());
    DeckView<size_t> v2c = v3c.child();
    do {
      TEST_ASSERT_EQUAL_UINT8(2, v2c.depth());
      DeckView<size_t> v1c = v2c.child();
      do {
        TEST_ASSERT_EQUAL_UINT8(1, v1c.depth());
        TEST_ASSERT_EQUAL_UINT64(1, v1c.size());
        std::span<size_t const> s = v1c.as_slice();
        for (size_t const &val : s) {
          TEST_ASSERT_EQUAL_UINT64(counter, val);
          ++counter;
        }
      } while (v1c.advance());
    } while (v2c.advance());
  } while (v3c.advance());
  TEST_ASSERT_EQUAL_UINT64(8, counter);
  // Graft then flatten roundtrip: items survive.
  Deck<size_t> d2 = binary_deck(2);
  std::vector<size_t> items2(d2.items().begin(), d2.items().end());
  d2.graft();
  d2.flatten();
  std::span<size_t const> after2 = d2.items();
  TEST_ASSERT_EQUAL_UINT64(items2.size(), after2.size());
  for (size_t i = 0; i < items2.size(); ++i)
    TEST_ASSERT_EQUAL_UINT64(items2[i], after2[i]);
}

// ---------------------------------------------------------------------------
// DeckView: simplify with view verification (t_deck_simplify)
// ---------------------------------------------------------------------------

static void test_simplify_with_view()
{
  // A deck whose mark depths already use every level is unchanged.
  {
    Deck<size_t> d = binary_deck(3);
    std::vector<uint8_t> depths_before;
    for (OrcMark const &m : d.marks()) depths_before.push_back(m.depth);
    d.simplify();
    std::vector<uint8_t> depths_after;
    for (OrcMark const &m : d.marks()) depths_after.push_back(m.depth);
    TEST_ASSERT_TRUE(depths_before == depths_after);
  }
  // A deck with gaps: only external depths 0, 2, 5 present.
  {
    Deck<size_t> d;
    d.push(0, 5);
    d.push(1, 0);
    d.push(2, 2);
    d.push(3, 0);
    std::vector<uint8_t> depths_before;
    for (OrcMark const &m : d.marks()) depths_before.push_back(m.depth);
    TEST_ASSERT_TRUE((depths_before == std::vector<uint8_t>{4, 1}));
    d.simplify();
    TEST_ASSERT_EQUAL_UINT8(2, d.max_depth());
    std::vector<uint8_t> depths_after;
    for (OrcMark const &m : d.marks()) depths_after.push_back(m.depth);
    TEST_ASSERT_TRUE((depths_after == std::vector<uint8_t>{1, 0}));
    // Iteration still works correctly after remapping.
    size_t count = 0;
    DeckView<size_t> v2 = d.view(2);
    if (v2.empty()) { do { ++count; } while (v2.advance()); }
    TEST_ASSERT_EQUAL_UINT64(1, count);
    size_t counter = 0;
    DeckView<size_t> v = d.view(2);
    DeckView<size_t> c = v.child();
    do {
      std::span<size_t const> s = c.as_slice();
      for (size_t const &val : s) {
        TEST_ASSERT_EQUAL_UINT64(counter, val);
        ++counter;
      }
    } while (c.advance());
    TEST_ASSERT_EQUAL_UINT64(4, counter);
    // Simplify is idempotent.
    d.simplify();
    std::vector<uint8_t> depths_again;
    for (OrcMark const &m : d.marks()) depths_again.push_back(m.depth);
    TEST_ASSERT_TRUE((depths_again == std::vector<uint8_t>{1, 0}));
  }
}

// ---------------------------------------------------------------------------
// DeckView: empty lists (t_empty_lists)
// ---------------------------------------------------------------------------

static void test_empty_lists()
{
  // Depth-1 empty list: a list with no items.
  {
    Deck<size_t> d;
    d.start_new_arr(1);
    TEST_ASSERT_EQUAL_UINT64(0, d.size());
    TEST_ASSERT_EQUAL_UINT8(1, d.max_depth());
    TEST_ASSERT_EQUAL_UINT64(1, d.marks().size());
    // Iterating at depth 1 yields one group with zero items.
    DeckView<size_t> v = d.view(1);
    TEST_ASSERT_TRUE(v.empty());
    TEST_ASSERT_EQUAL_UINT64(0, v.as_slice().size());
    TEST_ASSERT_EQUAL_UINT64(0, v.size());
    TEST_ASSERT_FALSE(v.advance());
  }
  // Empty list between non-empty lists at depth 2: ((1, 2), (), (3, 4))
  {
    Deck<size_t> d;
    d.push(1, 2);
    d.push(2, 0);
    d.start_new_arr(1);
    d.push(3, 1);
    d.push(4, 0);
    TEST_ASSERT_EQUAL_UINT64(4, d.size());
    TEST_ASSERT_EQUAL_UINT8(2, d.max_depth());
    DeckView<size_t> outer = d.view(2);
    size_t outer_count = 0;
    if (outer.empty()) { do { ++outer_count; } while (outer.advance()); }
    TEST_ASSERT_EQUAL_UINT64(1, outer_count);
    DeckView<size_t> o = d.view(2);
    DeckView<size_t> inner = o.child();
    std::span<size_t const> g0 = inner.as_slice();
    TEST_ASSERT_EQUAL_UINT64(2, g0.size());
    TEST_ASSERT_EQUAL_UINT64(1, g0[0]);
    TEST_ASSERT_EQUAL_UINT64(2, g0[1]);
    inner.advance();
    std::span<size_t const> g1 = inner.as_slice();
    TEST_ASSERT_EQUAL_UINT64(0, g1.size());
    inner.advance();
    std::span<size_t const> g2 = inner.as_slice();
    TEST_ASSERT_EQUAL_UINT64(2, g2.size());
    TEST_ASSERT_EQUAL_UINT64(3, g2[0]);
    TEST_ASSERT_EQUAL_UINT64(4, g2[1]);
  }
  // Empty list at the beginning: ((), (1, 2), (3, 4))
  {
    Deck<size_t> d;
    d.start_new_arr(2);
    d.push(1, 1);
    d.push(2, 0);
    d.push(3, 1);
    d.push(4, 0);
    DeckView<size_t> o = d.view(2);
    DeckView<size_t> inner = o.child();
    TEST_ASSERT_EQUAL_UINT64(0, inner.as_slice().size());
    inner.advance();
    TEST_ASSERT_EQUAL_UINT64(2, inner.as_slice().size());
    TEST_ASSERT_EQUAL_UINT64(1, inner.as_slice()[0]);
    inner.advance();
    TEST_ASSERT_EQUAL_UINT64(2, inner.as_slice().size());
    TEST_ASSERT_EQUAL_UINT64(3, inner.as_slice()[0]);
  }
  // Empty list at the end: ((1, 2), (3, 4), ())
  {
    Deck<size_t> d;
    d.push(1, 2);
    d.push(2, 0);
    d.push(3, 1);
    d.push(4, 0);
    d.start_new_arr(1);
    DeckView<size_t> o = d.view(2);
    DeckView<size_t> inner = o.child();
    TEST_ASSERT_EQUAL_UINT64(2, inner.as_slice().size());
    TEST_ASSERT_EQUAL_UINT64(1, inner.as_slice()[0]);
    inner.advance();
    TEST_ASSERT_EQUAL_UINT64(2, inner.as_slice().size());
    TEST_ASSERT_EQUAL_UINT64(3, inner.as_slice()[0]);
    inner.advance();
    TEST_ASSERT_EQUAL_UINT64(0, inner.as_slice().size());
  }
  // Multiple consecutive empty lists: ((), (), ())
  {
    Deck<size_t> d;
    d.start_new_arr(2);
    d.start_new_arr(1);
    d.start_new_arr(1);
    TEST_ASSERT_EQUAL_UINT64(0, d.size());
    TEST_ASSERT_EQUAL_UINT8(2, d.max_depth());
    DeckView<size_t> o = d.view(2);
    size_t n_inner = 0;
    DeckView<size_t> inner = o.child();
    do {
      TEST_ASSERT_EQUAL_UINT64(0, inner.as_slice().size());
      ++n_inner;
    } while (inner.advance());
    TEST_ASSERT_EQUAL_UINT64(3, n_inner);
  }
  // Depth-3 with nested empty: (((1, 2), ()), ((3, 4)))
  {
    Deck<size_t> d;
    d.push(1, 3);
    d.push(2, 0);
    d.start_new_arr(1);
    d.push(3, 2);
    d.push(4, 0);
    TEST_ASSERT_EQUAL_UINT8(3, d.max_depth());
    DeckView<size_t> top = d.view(3);
    DeckView<size_t> mid = top.child();
    // First mid: ((1, 2), ())
    DeckView<size_t> fi = mid.child();
    TEST_ASSERT_EQUAL_UINT64(2, fi.as_slice().size());
    TEST_ASSERT_EQUAL_UINT64(1, fi.as_slice()[0]);
    fi.advance();
    TEST_ASSERT_EQUAL_UINT64(0, fi.as_slice().size());
    // Second mid: ((3, 4))
    mid.advance();
    DeckView<size_t> si = mid.child();
    TEST_ASSERT_EQUAL_UINT64(2, si.as_slice().size());
    TEST_ASSERT_EQUAL_UINT64(3, si.as_slice()[0]);
  }
  // Flatten preserves items but drops empty structure.
  {
    Deck<size_t> d;
    d.push(1, 2);
    d.push(2, 0);
    d.start_new_arr(1);
    d.push(3, 1);
    d.push(4, 0);
    d.flatten();
    TEST_ASSERT_EQUAL_UINT8(1, d.max_depth());
    DeckView<size_t> v = d.view(1);
    std::span<size_t const> s = v.as_slice();
    TEST_ASSERT_EQUAL_UINT64(4, s.size());
    TEST_ASSERT_EQUAL_UINT64(1, s[0]);
    TEST_ASSERT_EQUAL_UINT64(2, s[1]);
    TEST_ASSERT_EQUAL_UINT64(3, s[2]);
    TEST_ASSERT_EQUAL_UINT64(4, s[3]);
  }
}

// ---------------------------------------------------------------------------
// DeckView: as_slice test
// ---------------------------------------------------------------------------

static void test_view_as_slice()
{
  Deck<int> d = Deck<int>::build<2>({{1, 2, 3}, {4, 5, 6}});
  DeckView<int> v = d.view(2);
  DeckView<int> child = v.child();
  std::span<int const> slice = child.as_slice();
  TEST_ASSERT_EQUAL_UINT64(3, slice.size());
  TEST_ASSERT_EQUAL_INT(1, slice[0]);
  TEST_ASSERT_EQUAL_INT(2, slice[1]);
  TEST_ASSERT_EQUAL_INT(3, slice[2]);
  child.advance();
  slice = child.as_slice();
  TEST_ASSERT_EQUAL_UINT64(3, slice.size());
  TEST_ASSERT_EQUAL_INT(4, slice[0]);
  TEST_ASSERT_EQUAL_INT(5, slice[1]);
  TEST_ASSERT_EQUAL_INT(6, slice[2]);
}

// ---------------------------------------------------------------------------
// DeckView: intermediate depth (t_view_intermediate_depth)
// ---------------------------------------------------------------------------

static void test_view_intermediate_depth()
{
  // Build depth-3 tree: {{{0,1},{2,3}}, {{4,5},{6,7}}}
  Deck<uint32_t> d;
  {
    DeckWriter<uint32_t> w = d.writer(3);
    for (uint32_t i = 0; i < 2; ++i) {
      DeckWriter<uint32_t> g2 = w.child();
      for (uint32_t j = 0; j < 2; ++j) {
        DeckWriter<uint32_t> g1 = g2.child();
        g1.push(i * 4 + j * 2);
        g1.push(i * 4 + j * 2 + 1);
      }
    }
  }
  // depth-3 view: 2 groups of 2 groups of 2
  std::vector<std::vector<std::vector<uint32_t>>> t3 = tree3(d);
  TEST_ASSERT_EQUAL_UINT64(2, t3.size());
  TEST_ASSERT_TRUE((t3[0] == std::vector<std::vector<uint32_t>>{{0, 1}, {2, 3}}));
  TEST_ASSERT_TRUE((t3[1] == std::vector<std::vector<uint32_t>>{{4, 5}, {6, 7}}));
  // depth-2 view: 4 groups of 2
  std::vector<std::vector<uint32_t>> t2 = tree2(d);
  TEST_ASSERT_EQUAL_UINT64(4, t2.size());
  TEST_ASSERT_TRUE((t2[0] == std::vector<uint32_t>{0, 1}));
  TEST_ASSERT_TRUE((t2[1] == std::vector<uint32_t>{2, 3}));
  TEST_ASSERT_TRUE((t2[2] == std::vector<uint32_t>{4, 5}));
  TEST_ASSERT_TRUE((t2[3] == std::vector<uint32_t>{6, 7}));
  // depth-1 view: 4 groups of 2 (each mark starts a depth-1 group)
  std::vector<std::vector<uint32_t>> flat;
  DeckView<uint32_t> v1 = d.view(1);
  if (v1.empty()) {
    do {
      std::span<uint32_t const> s = v1.as_slice();
      flat.push_back(std::vector<uint32_t>(s.begin(), s.end()));
    } while (v1.advance());
  }
  TEST_ASSERT_EQUAL_UINT64(4, flat.size());
  TEST_ASSERT_TRUE((flat[0] == std::vector<uint32_t>{0, 1}));
  TEST_ASSERT_TRUE((flat[1] == std::vector<uint32_t>{2, 3}));
  TEST_ASSERT_TRUE((flat[2] == std::vector<uint32_t>{4, 5}));
  TEST_ASSERT_TRUE((flat[3] == std::vector<uint32_t>{6, 7}));
}

// ---------------------------------------------------------------------------
// DeckWriter: basic 3x3x3 (t_deck_writer_basic)
// ---------------------------------------------------------------------------

static void test_writer_3x3x3()
{
  Deck<uint32_t> d;
  uint32_t counter = 0;
  {
    DeckWriter<uint32_t> dst3 = d.writer(3);
    TEST_ASSERT_EQUAL_UINT8(3, dst3.depth());
    for (int a = 0; a < 3; ++a) {
      DeckWriter<uint32_t> dst2 = dst3.child();
      TEST_ASSERT_EQUAL_UINT8(2, dst2.depth());
      for (int b = 0; b < 3; ++b) {
        DeckWriter<uint32_t> dst1 = dst2.child();
        TEST_ASSERT_EQUAL_UINT8(1, dst1.depth());
        for (int c = 0; c < 3; ++c) {
          dst1.push(counter);
          ++counter;
        }
        TEST_ASSERT_EQUAL_UINT64(3, dst1.size());
        TEST_ASSERT_EQUAL_UINT32(counter - 3, dst1[0]);
        TEST_ASSERT_EQUAL_UINT32(counter - 1, dst1[2]);
        dst1[1] = counter - 2 + 100;
        dst1[1] = counter - 2;  // restore
      }
    }
  }
  // Read and check via tree3.
  std::vector<std::vector<std::vector<uint32_t>>> expected = {
    {{0, 1, 2}, {3, 4, 5}, {6, 7, 8}},
    {{9, 10, 11}, {12, 13, 14}, {15, 16, 17}},
    {{18, 19, 20}, {21, 22, 23}, {24, 25, 26}},
  };
  TEST_ASSERT_TRUE(tree3(d) == expected);
  // Verify via view iteration with Index.
  counter = 0;
  DeckView<uint32_t> v3 = d.view(3);
  do {
    DeckView<uint32_t> v2c = v3.child();
    do {
      TEST_ASSERT_EQUAL_UINT32(counter, v2c[0]);
      DeckView<uint32_t> v1c = v2c.child();
      do {
        TEST_ASSERT_EQUAL_UINT32(counter, v1c[0]);
        std::span<uint32_t const> arr = v1c.as_slice();
        for (uint32_t const &val : arr) {
          TEST_ASSERT_EQUAL_UINT32(counter, val);
          ++counter;
        }
      } while (v1c.advance());
    } while (v2c.advance());
  } while (v3.advance());
}

// ---------------------------------------------------------------------------
// DeckWriter: with extend (t_deck_writer_with_extend)
// ---------------------------------------------------------------------------

static void test_writer_with_extend()
{
  Deck<uint32_t> d;
  uint32_t counter = 0;
  {
    DeckWriter<uint32_t> dst3 = d.writer(3);
    TEST_ASSERT_EQUAL_UINT8(3, dst3.depth());
    for (int a = 0; a < 3; ++a) {
      DeckWriter<uint32_t> dst2 = dst3.child();
      TEST_ASSERT_EQUAL_UINT8(2, dst2.depth());
      for (int b = 0; b < 3; ++b) {
        DeckWriter<uint32_t> dst1 = dst2.child();
        TEST_ASSERT_EQUAL_UINT8(1, dst1.depth());
        uint32_t items[] = {counter, counter + 1, counter + 2};
        dst1.extend_from_slice(items);
        counter += 3;
        TEST_ASSERT_EQUAL_UINT64(3, dst1.size());
        std::span<uint32_t> sm = dst1.as_slice_mut();
        TEST_ASSERT_EQUAL_UINT64(3, sm.size());
        TEST_ASSERT_EQUAL_UINT32(counter - 3, sm[0]);
      }
    }
  }
  // Read and check.
  counter = 0;
  DeckView<uint32_t> v3 = d.view(3);
  do {
    DeckView<uint32_t> v2c = v3.child();
    do {
      DeckView<uint32_t> v1c = v2c.child();
      do {
        std::span<uint32_t const> arr = v1c.as_slice();
        for (uint32_t const &val : arr) {
          TEST_ASSERT_EQUAL_UINT32(counter, val);
          ++counter;
        }
      } while (v1c.advance());
    } while (v2c.advance());
  } while (v3.advance());
}

// ---------------------------------------------------------------------------
// DeckWriter: unbalanced tree (t_unbalanced_tree)
// ---------------------------------------------------------------------------

static void test_writer_unbalanced()
{
  // ((1), (2, 3, 4), (5, 6))
  Deck<uint32_t> d;
  {
    DeckWriter<uint32_t> dst = d.writer(2);
    { DeckWriter<uint32_t> c = dst.child(); c.push(1); }
    { DeckWriter<uint32_t> c = dst.child(); c.push(2); c.push(3); c.push(4); }
    { DeckWriter<uint32_t> c = dst.child(); c.push(5); c.push(6); }
  }
  std::vector<std::vector<uint32_t>> expected = {{1}, {2, 3, 4}, {5, 6}};
  TEST_ASSERT_TRUE(tree2(d) == expected);
}

// ---------------------------------------------------------------------------
// DeckWriter: empty groups (t_empty_groups_via_deck_writer)
// ---------------------------------------------------------------------------

static void test_writer_empty_groups()
{
  // ((), (1, 2), (), (3), ())
  Deck<uint32_t> d;
  {
    DeckWriter<uint32_t> dst = d.writer(2);
    { DeckWriter<uint32_t> c = dst.child(); }  // empty
    { DeckWriter<uint32_t> c = dst.child(); c.push(1); c.push(2); }
    { DeckWriter<uint32_t> c = dst.child(); }  // empty
    { DeckWriter<uint32_t> c = dst.child(); c.push(3); }
    { DeckWriter<uint32_t> c = dst.child(); }  // empty
  }
  std::vector<std::vector<uint32_t>> expected = {{}, {1, 2}, {}, {3}, {}};
  TEST_ASSERT_TRUE(tree2(d) == expected);
  // Items are just 1, 2, 3.
  TEST_ASSERT_EQUAL_UINT64(3, d.size());
  TEST_ASSERT_EQUAL_UINT32(1, d[0]);
  TEST_ASSERT_EQUAL_UINT32(2, d[1]);
  TEST_ASSERT_EQUAL_UINT32(3, d[2]);
}

// ---------------------------------------------------------------------------
// DeckWriter: nested empty (t_nested_empty_via_deck_writer)
// ---------------------------------------------------------------------------

static void test_writer_nested_empty()
{
  // (((), (1, 2)), ((3,)), ((),))
  Deck<uint32_t> d;
  {
    DeckWriter<uint32_t> dst3 = d.writer(3);
    {
      DeckWriter<uint32_t> dst2 = dst3.child();
      { DeckWriter<uint32_t> c = dst2.child(); }  // empty inner
      { DeckWriter<uint32_t> c = dst2.child(); c.push(1); c.push(2); }
    }
    {
      DeckWriter<uint32_t> dst2 = dst3.child();
      { DeckWriter<uint32_t> c = dst2.child(); c.push(3); }
    }
    {
      DeckWriter<uint32_t> dst2 = dst3.child();
      { DeckWriter<uint32_t> c = dst2.child(); }  // empty inner
    }
  }
  std::vector<std::vector<std::vector<uint32_t>>> expected = {
    {{}, {1, 2}},
    {{3}},
    {{}},
  };
  TEST_ASSERT_TRUE(tree3(d) == expected);
}

// ---------------------------------------------------------------------------
// DeckWriter: extend with empty (t_extend_empty_iterator)
// ---------------------------------------------------------------------------

static void test_writer_extend_empty()
{
  Deck<uint32_t> d;
  {
    DeckWriter<uint32_t> dst = d.writer(2);
    {
      DeckWriter<uint32_t> c = dst.child();
      uint32_t items[] = {1, 2};
      c.extend_from_slice(items);
    }
    {
      DeckWriter<uint32_t> c = dst.child();
      c.extend_from_slice(std::span<uint32_t const>());  // empty extend
    }
    {
      DeckWriter<uint32_t> c = dst.child();
      uint32_t items[] = {3};
      c.extend_from_slice(items);
    }
  }
  std::vector<std::vector<uint32_t>> expected = {{1, 2}, {}, {3}};
  TEST_ASSERT_TRUE(tree2(d) == expected);
}

// ---------------------------------------------------------------------------
// DeckWriter: single element deep (t_single_element_deep)
// ---------------------------------------------------------------------------

static void test_writer_single_deep()
{
  // One item wrapped at depth 5: (((((42)))))
  Deck<uint32_t> d;
  {
    DeckWriter<uint32_t> v5 = d.writer(5);
    DeckWriter<uint32_t> v4 = v5.child();
    DeckWriter<uint32_t> v3 = v4.child();
    DeckWriter<uint32_t> v2 = v3.child();
    DeckWriter<uint32_t> v1 = v2.child();
    v1.push(42);
  }
  TEST_ASSERT_EQUAL_UINT8(5, d.max_depth());
  TEST_ASSERT_EQUAL_UINT64(1, d.size());
  // Unwrap all the way down via children.
  DeckView<uint32_t> vv5 = d.view(5);
  DeckView<uint32_t> vv4 = vv5.child();
  DeckView<uint32_t> vv3 = vv4.child();
  DeckView<uint32_t> vv2 = vv3.child();
  DeckView<uint32_t> vv1 = vv2.child();
  std::span<uint32_t const> leaf = vv1.as_slice();
  TEST_ASSERT_EQUAL_UINT64(1, leaf.size());
  TEST_ASSERT_EQUAL_UINT32(42, leaf[0]);
}

// ---------------------------------------------------------------------------
// DeckWriter: append to existing (t_append_to_existing)
// ---------------------------------------------------------------------------

static void test_writer_append_to_existing()
{
  Deck<uint32_t> d;
  d.push(1, 2);
  d.push(2, 0);
  d.push(3, 1);
  d.push(4, 0);
  // deck is ((1,2),(3,4)) at depth 2
  std::vector<std::vector<uint32_t>> before = {{1, 2}, {3, 4}};
  TEST_ASSERT_TRUE(tree2(d) == before);
  // Append another group at depth 1 (same outer group).
  {
    DeckWriter<uint32_t> dst = d.writer(1);
    dst.push(5);
    dst.push(6);
  }
  // Now: ((1,2),(3,4),(5,6))
  std::vector<std::vector<uint32_t>> after = {{1, 2}, {3, 4}, {5, 6}};
  TEST_ASSERT_TRUE(tree2(d) == after);
}

// ---------------------------------------------------------------------------
// DeckWriter: graft/simplify after writer (t_graft_simplify_after_deck_writer)
// ---------------------------------------------------------------------------

static void test_writer_graft_simplify()
{
  Deck<uint32_t> d;
  {
    DeckWriter<uint32_t> dst = d.writer(2);
    { DeckWriter<uint32_t> c = dst.child(); c.push(1); c.push(2); }
    { DeckWriter<uint32_t> c = dst.child(); c.push(3); }
  }
  std::vector<std::vector<uint32_t>> before = {{1, 2}, {3}};
  TEST_ASSERT_TRUE(tree2(d) == before);
  // Graft: depth 2 → 3
  d.graft();
  TEST_ASSERT_EQUAL_UINT8(3, d.max_depth());
  std::vector<std::vector<std::vector<uint32_t>>> after_graft = {
    {{1}, {2}},
    {{3}},
  };
  TEST_ASSERT_TRUE(tree3(d) == after_graft);
  // Simplify should be no-op (depths are already contiguous).
  std::vector<uint8_t> depths_before;
  for (OrcMark const &m : d.marks()) depths_before.push_back(m.depth);
  d.simplify();
  std::vector<uint8_t> depths_after;
  for (OrcMark const &m : d.marks()) depths_after.push_back(m.depth);
  TEST_ASSERT_TRUE(depths_before == depths_after);
  // Flatten.
  d.flatten();
  TEST_ASSERT_EQUAL_UINT8(1, d.max_depth());
  TEST_ASSERT_EQUAL_UINT32(1, d[0]);
  TEST_ASSERT_EQUAL_UINT32(2, d[1]);
  TEST_ASSERT_EQUAL_UINT32(3, d[2]);
}

// ---------------------------------------------------------------------------
// DeckWriter: all empty (t_all_empty_via_deck_writer)
// ---------------------------------------------------------------------------

static void test_writer_all_empty()
{
  // Depth-3 tree with no items: (((), ()), (()))
  Deck<uint32_t> d;
  {
    DeckWriter<uint32_t> dst3 = d.writer(3);
    {
      DeckWriter<uint32_t> dst2 = dst3.child();
      { DeckWriter<uint32_t> c = dst2.child(); }
      { DeckWriter<uint32_t> c = dst2.child(); }
    }
    {
      DeckWriter<uint32_t> dst2 = dst3.child();
      { DeckWriter<uint32_t> c = dst2.child(); }
    }
  }
  TEST_ASSERT_EQUAL_UINT64(0, d.size());
  TEST_ASSERT_EQUAL_UINT8(3, d.max_depth());
  std::vector<std::vector<std::vector<uint32_t>>> expected = {
    {{}, {}},
    {{}},
  };
  TEST_ASSERT_TRUE(tree3(d) == expected);
}

// ---------------------------------------------------------------------------
// DeckWriter: len at each level (t_deck_riter_len_at_each_level)
// ---------------------------------------------------------------------------

static void test_writer_len_at_each_level()
{
  Deck<uint32_t> d;
  {
    DeckWriter<uint32_t> dst3 = d.writer(3);
    TEST_ASSERT_EQUAL_UINT64(0, dst3.size());
    {
      DeckWriter<uint32_t> dst2 = dst3.child();
      TEST_ASSERT_EQUAL_UINT64(0, dst2.size());
      {
        DeckWriter<uint32_t> dst1 = dst2.child();
        TEST_ASSERT_EQUAL_UINT64(0, dst1.size());
        dst1.push(10);
        TEST_ASSERT_EQUAL_UINT64(1, dst1.size());
        dst1.push(20);
        TEST_ASSERT_EQUAL_UINT64(2, dst1.size());
      }
      TEST_ASSERT_EQUAL_UINT64(2, dst2.size());
      {
        DeckWriter<uint32_t> dst1 = dst2.child();
        dst1.push(30);
        TEST_ASSERT_EQUAL_UINT64(1, dst1.size());
      }
      TEST_ASSERT_EQUAL_UINT64(3, dst2.size());
    }
    TEST_ASSERT_EQUAL_UINT64(3, dst3.size());
    {
      DeckWriter<uint32_t> dst2 = dst3.child();
      TEST_ASSERT_EQUAL_UINT64(0, dst2.size());
      {
        DeckWriter<uint32_t> dst1 = dst2.child();
        dst1.push(40);
      }
      TEST_ASSERT_EQUAL_UINT64(1, dst2.size());
    }
    TEST_ASSERT_EQUAL_UINT64(4, dst3.size());
  }
  TEST_ASSERT_EQUAL_UINT64(4, d.size());
}

// ---------------------------------------------------------------------------
// DeckWriter: basic depth-2 (original test)
// ---------------------------------------------------------------------------

static void test_writer_basic()
{
  Deck<int> d;
  {
    DeckWriter<int> w = d.writer(2);
    { DeckWriter<int> g1 = w.child(); g1.push(1); g1.push(2); }
    { DeckWriter<int> g2 = w.child(); g2.push(3); g2.push(4); }
  }
  Deck<int> expected = Deck<int>::build<2>({{1, 2}, {3, 4}});
  TEST_ASSERT_TRUE(d == expected);
}

// ---------------------------------------------------------------------------
// DeckWriter: basic depth-3 (original test)
// ---------------------------------------------------------------------------

static void test_writer_depth3()
{
  Deck<int> d;
  {
    DeckWriter<int> w = d.writer(3);
    {
      DeckWriter<int> g1 = w.child();
      { DeckWriter<int> i = g1.child(); i.push(1); i.push(2); }
      { DeckWriter<int> i = g1.child(); i.push(3); i.push(4); }
    }
    {
      DeckWriter<int> g2 = w.child();
      { DeckWriter<int> i = g2.child(); i.push(5); i.push(6); }
      { DeckWriter<int> i = g2.child(); i.push(7); i.push(8); }
    }
  }
  Deck<int> expected = Deck<int>::build<3>({{{1, 2}, {3, 4}}, {{5, 6}, {7, 8}}});
  TEST_ASSERT_TRUE(d == expected);
}

// ---------------------------------------------------------------------------
// DeckWriter: roundtrip (build + view)
// ---------------------------------------------------------------------------

static void test_writer_roundtrip()
{
  Deck<int> d;
  {
    DeckWriter<int> w = d.writer(2);
    { DeckWriter<int> g = w.child(); g.push(1); g.push(2); g.push(3); }
    { DeckWriter<int> g = w.child(); g.push(4); g.push(5); g.push(6); }
  }
  DeckView<int> v = d.view(2);
  DeckView<int> c1 = v.child();
  std::span<int const> s1 = c1.as_slice();
  TEST_ASSERT_EQUAL_UINT64(3, s1.size());
  TEST_ASSERT_EQUAL_INT(1, s1[0]);
  TEST_ASSERT_EQUAL_INT(2, s1[1]);
  TEST_ASSERT_EQUAL_INT(3, s1[2]);
  c1.advance();
  std::span<int const> s2 = c1.as_slice();
  TEST_ASSERT_EQUAL_UINT64(3, s2.size());
  TEST_ASSERT_EQUAL_INT(4, s2[0]);
  TEST_ASSERT_EQUAL_INT(5, s2[1]);
  TEST_ASSERT_EQUAL_INT(6, s2[2]);
}

// ---------------------------------------------------------------------------

void setUp(void) {}
void tearDown(void) {}

int main(void)
{
  UNITY_BEGIN();
  // Build
  RUN_TEST(test_build_flat);
  RUN_TEST(test_build_depth2);
  RUN_TEST(test_build_depth3);
  RUN_TEST(test_build_depth3_3way);
  RUN_TEST(test_build_single);
  RUN_TEST(test_build_empty_groups);
  RUN_TEST(test_build_ragged);
  // Mutation
  RUN_TEST(test_flatten);
  RUN_TEST(test_graft);
  RUN_TEST(test_simplify);
  RUN_TEST(test_clear);
  RUN_TEST(test_push_manual);
  // Display
  RUN_TEST(test_display_empty);
  RUN_TEST(test_display_flat);
  RUN_TEST(test_display_depth2);
  RUN_TEST(test_display_depth3);
  RUN_TEST(test_display_single);
  RUN_TEST(test_display_empty_group);
  // DeckView
  RUN_TEST(test_view_basic_depth5);
  RUN_TEST(test_view_from_level4);
  RUN_TEST(test_view_from_level3);
  RUN_TEST(test_view_edge_cases);
  RUN_TEST(test_view_as_slice);
  RUN_TEST(test_view_intermediate_depth);
  RUN_TEST(test_flatten_with_view);
  RUN_TEST(test_graft_with_view);
  RUN_TEST(test_simplify_with_view);
  RUN_TEST(test_empty_lists);
  // DeckWriter
  RUN_TEST(test_writer_basic);
  RUN_TEST(test_writer_depth3);
  RUN_TEST(test_writer_3x3x3);
  RUN_TEST(test_writer_with_extend);
  RUN_TEST(test_writer_unbalanced);
  RUN_TEST(test_writer_empty_groups);
  RUN_TEST(test_writer_nested_empty);
  RUN_TEST(test_writer_extend_empty);
  RUN_TEST(test_writer_single_deep);
  RUN_TEST(test_writer_append_to_existing);
  RUN_TEST(test_writer_graft_simplify);
  RUN_TEST(test_writer_all_empty);
  RUN_TEST(test_writer_len_at_each_level);
  RUN_TEST(test_writer_roundtrip);
  return UNITY_END();
}
