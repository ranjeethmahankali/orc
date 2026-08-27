#pragma once

extern "C"
{
#include "orc_abi.h"
}
#include "error.hpp"

#include <algorithm>
#include <cstddef>
#include <cstdint>
#include <initializer_list>
#include <ostream>
#include <span>
#include <vector>

namespace orc_sdk {

// Maps a depth to the corresponding nested std::initializer_list type.
template<typename T, uint8_t Depth>
struct NestedInitList
{
  using type = std::initializer_list<typename NestedInitList<T, Depth - 1>::type>;
};

template<typename T>
struct NestedInitList<T, 1>
{
  using type = std::initializer_list<T>;
};

size_t calc_stride_count(std::vector<OrcMark> const  &marks,
                         std::vector<uint64_t> const &stride_offset);

void calc_strides(std::vector<OrcMark> const &marks,
                  std::vector<size_t>        &pegs,
                  std::vector<uint64_t>      &stride_offset,
                  std::vector<uint64_t>      &strides);

template<typename T>
class Deck
{
  std::vector<T>        m_items;
  std::vector<OrcMark>  m_marks;
  std::vector<uint64_t> m_stride_offset;
  std::vector<uint64_t> m_strides;
  std::vector<size_t>   m_pegs;

  void recalc_strides() { calc_strides(m_marks, m_pegs, m_stride_offset, m_strides); }

  void push_mark(OrcMark mark)
  {
    // Ensure no mark exceeds the first mark's depth.
    uint8_t depth =
      m_marks.empty() ? mark.depth : std::min(m_marks.front().depth, mark.depth);
    mark.depth = depth;
    size_t d   = static_cast<size_t>(depth);
    if (d > m_pegs.size())
      m_pegs.resize(d, 0);
    // Update the scan then push the actual marker.
    m_stride_offset.push_back(
      static_cast<uint64_t>(calc_stride_count(m_marks, m_stride_offset)));
    // Update strides.
    m_strides.resize(m_strides.size() + d, UINT64_MAX);
    for (size_t i = 0; i < d; ++i) {
      size_t peg = m_pegs[i];
      if (peg < m_marks.size()) {
        uint64_t &dst = m_strides[static_cast<size_t>(m_stride_offset[peg]) + i];
        dst       = std::min(dst, static_cast<uint64_t>(m_marks.size() - peg));
      }
      m_pegs[i] = m_marks.size();
    }
    m_marks.push_back(mark);
  }

public:
  Deck() = default;

  static Deck from_raw_data(std::vector<T> items, std::vector<OrcMark> marks)
  {
    Deck out;
    out.m_items = std::move(items);
    out.m_marks = std::move(marks);
    out.recalc_strides();
    return out;
  }

  static Deck from_value(T item)
  {
    Deck out;
    out.m_items.push_back(std::move(item));
    out.recalc_strides();
    return out;
  }

  // Terminal case: list of T values.
  template<uint8_t>
  static void build_impl(Deck &d, std::initializer_list<T> list, uint8_t depth)
  {
    if (list.size() == 0) {
      d.start_new_arr(depth);
      return;
    }
    for (T const &item : list) {
      d.push(item, depth);
      depth = 0;
    }
  }

  // Recursive case: list of lists.
  template<uint8_t GroupDepth, typename Inner>
  static void build_impl(Deck &d, std::initializer_list<Inner> list, uint8_t depth)
  {
    for (Inner const &group : list) {
      build_impl<GroupDepth - 1>(d, group, depth);
      depth = GroupDepth - 1;
    }
  }

  template<uint8_t Depth = 1>
  static Deck build(typename NestedInitList<T, Depth>::type list)
  {
    Deck d;
    build_impl<Depth>(d, list, Depth);
    return d;
  }

  void assign_from_raw_data(std::vector<T> items, std::vector<OrcMark> marks)
  {
    m_items = std::move(items);
    m_marks = std::move(marks);
    m_stride_offset.clear();
    m_strides.clear();
    m_pegs.clear();
    recalc_strides();
  }

  uint8_t max_depth() const
  {
    return m_marks.empty() ? 0 : static_cast<uint8_t>(m_marks.front().depth + 1);
  }

  size_t len() const { return m_items.size(); }

  bool is_empty() const { return m_items.empty(); }

  void push(T item, uint8_t depth)
  {
    start_new_arr(depth);
    m_items.push_back(std::move(item));
  }

  void start_new_arr(uint8_t depth)
  {
    if (depth > 0) {
      push_mark(OrcMark {.depth = static_cast<uint8_t>(depth - 1),
                         .pos   = static_cast<uint64_t>(m_items.size())});
    }
  }

  void clear()
  {
    m_items.clear();
    m_marks.clear();
    m_stride_offset.clear();
    m_strides.clear();
    m_pegs.clear();
  }

  void reserve(size_t additional)
  {
    m_items.reserve(m_items.size() + additional);
    m_marks.reserve(m_marks.size() + additional);
    m_stride_offset.reserve(m_stride_offset.size() + additional);
  }

  void flatten()
  {
    m_marks.clear();
    m_stride_offset.clear();
    m_strides.clear();
    m_pegs.clear();
    if (m_items.size() > 1) {
      push_mark(OrcMark {.depth = 0, .pos = 0});
    }
  }

  Error graft()
  {
    for (OrcMark const &m : m_marks) {
      if (m.depth >= 254)
        return Error::DECK_DEPTH_OVERFLOW;
    }
    size_t             count     = m_marks.size();
    std::vector<OrcMark> old_marks = std::move(m_marks);
    m_marks.clear();
    m_marks.reserve(count + m_items.size());
    uint64_t prev = 0;
    for (OrcMark &m : old_marks) {
      m.depth += 1;
      for (uint64_t pos = prev; pos < m.pos; ++pos) {
        m_marks.push_back(OrcMark {.depth = 0, .pos = pos});
      }
      m_marks.push_back(m);
      prev = m.pos + 1;
    }
    for (uint64_t pos = prev; pos < static_cast<uint64_t>(m_items.size()); ++pos) {
      m_marks.push_back(OrcMark {.depth = 0, .pos = pos});
    }
    recalc_strides();
    return Error::NONE;
  }

  void simplify()
  {
    if (m_marks.empty())
      return;
    size_t               dmax = static_cast<size_t>(m_marks.front().depth);
    std::vector<uint8_t> remap(dmax + 1, 0);
    for (OrcMark const &m : m_marks) {
      remap[m.depth] = 1;
    }
    uint8_t acc2 = 0;
    for (uint8_t &r : remap) {
      uint8_t prev2 = acc2;
      acc2 += r;
      r = prev2;
    }
    for (OrcMark &m : m_marks) {
      m.depth = remap[m.depth];
    }
    recalc_strides();
  }

  std::span<T const>        items() const { return m_items; }
  std::span<T>              items() { return m_items; }
  std::span<OrcMark const>  marks() const { return m_marks; }
  std::span<uint64_t const> stride_offset() const { return m_stride_offset; }
  std::span<uint64_t const> strides() const { return m_strides; }

  T const &operator[](size_t i) const { return m_items[i]; }
  T       &operator[](size_t i) { return m_items[i]; }

  bool operator==(Deck const &other) const
  {
    return m_items == other.m_items && m_marks.size() == other.m_marks.size() &&
           std::equal(m_marks.begin(),
                      m_marks.end(),
                      other.m_marks.begin(),
                      [](OrcMark const &a, OrcMark const &b) {
                        return a.depth == b.depth && a.pos == b.pos;
                      });
  }
};

template<typename T>
void fmt_raw_deck(std::span<T const>       items,
                  std::span<OrcMark const> marks,
                  std::ostream            &os)
{
  if (items.empty() && marks.empty()) {
    os << "<empty_deck>\n";
    return;
  }
  constexpr size_t TAB = 3;
  uint8_t  dmax    = marks.empty() ? uint8_t{0} : marks.front().depth;
  uint64_t n_items = static_cast<uint64_t>(items.size());
  uint64_t tail_start = 0;

  auto spaces = [&](size_t n) {
    for (size_t i = 0; i < n; ++i)
      os << ' ';
  };
  auto dashes = [&](size_t n) {
    for (size_t i = 0; i < n; ++i)
      os << '-';
  };
  auto write_depth = [&](uint8_t d) {
    if (d < 10)
      os << "  " << static_cast<int>(d);
    else if (d < 100)
      os << ' ' << static_cast<int>(d);
    else
      os << static_cast<int>(d);
  };
  auto write_continuation = [&](uint64_t idx) {
    spaces((static_cast<size_t>(dmax) + 1) * TAB);
    os << "    | " << items[idx] << '\n';
  };
  auto write_mark_line = [&](OrcMark const &m, uint64_t next_pos) {
    spaces(static_cast<size_t>(dmax - m.depth) * TAB);
    uint8_t d_current = static_cast<uint8_t>(m.depth + 1);
    write_depth(d_current);
    os << ' ';
    dashes(static_cast<size_t>(d_current) * TAB);
    os << '|';
    if (m.pos < next_pos) {
      uint64_t end = std::min(next_pos, n_items);
      os << ' ' << items[m.pos] << '\n';
      for (uint64_t i = m.pos + 1; i < end; ++i)
        write_continuation(i);
    }
    else {
      os << '\n';
    }
  };

  for (size_t i = 0; i + 1 < marks.size(); ++i) {
    write_mark_line(marks[i], marks[i + 1].pos);
    tail_start = marks[i + 1].pos;
  }
  if (!marks.empty()) {
    write_mark_line(marks.back(), n_items);
    tail_start = n_items;
  }
  for (uint64_t i = tail_start; i < n_items; ++i)
    write_continuation(i);
}

template<typename T>
std::ostream &operator<<(std::ostream &os, Deck<T> const &deck)
{
  fmt_raw_deck(deck.items(), deck.marks(), os);
  return os;
}

}  // namespace orc_sdk
