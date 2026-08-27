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
#include <optional>
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

size_t calc_stride_count(std::span<OrcMark const>  marks,
                         std::span<uint64_t const> stride_offset);

void calc_strides(std::vector<OrcMark> const &marks,
                  std::vector<size_t>        &pegs,
                  std::vector<uint64_t>      &stride_offset,
                  std::vector<uint64_t>      &strides);

// Forward declarations.
template<typename T>
class Deck;
template<typename T>
class DeckView;
template<typename T>
class DeckWriter;

// ---------------------------------------------------------------------------
// Free helper functions used by ReadCursor.
// ---------------------------------------------------------------------------

inline size_t stride(std::span<OrcMark const>  marks,
                     std::span<uint64_t const> strides,
                     std::span<uint64_t const> stride_offset,
                     size_t                    mark_idx,
                     uint8_t                   depth)
{
  if (mark_idx >= marks.size())
    return 0;
  if (depth == 0)
    return 1;
  if (depth > marks[mark_idx].depth)
    return marks.size() - mark_idx;
  uint64_t val = strides[static_cast<size_t>(stride_offset[mark_idx]) + (depth - 1)];
  return static_cast<size_t>(
    std::min(val, static_cast<uint64_t>(marks.size() - mark_idx)));
}

inline size_t mark_pos(std::span<OrcMark const> marks, size_t n_items, size_t idx)
{
  if (idx < marks.size())
    return static_cast<size_t>(marks[idx].pos);
  return n_items;
}

// ---------------------------------------------------------------------------
// ReadCursor
// ---------------------------------------------------------------------------

class ReadCursor
{
  size_t                    m_n_items;
  std::span<OrcMark const>  m_marks;
  std::span<uint64_t const> m_strides;
  std::span<uint64_t const> m_stride_offset;
  uint8_t                   m_depth;
  size_t                    m_start;
  size_t                    m_end;

public:
  ReadCursor(size_t                    n_items,
             std::span<OrcMark const>  marks,
             std::span<uint64_t const> strides,
             std::span<uint64_t const> stride_offset,
             uint8_t                   depth,
             size_t                    start,
             size_t                    end)
      : m_n_items(n_items)
      , m_marks(marks)
      , m_strides(strides)
      , m_stride_offset(stride_offset)
      , m_depth(depth)
      , m_start(start)
      , m_end(end)
  {}

  uint8_t                  depth() const { return m_depth; }
  size_t                   start() const { return m_start; }
  size_t                   end() const { return m_end; }
  bool                     empty() const { return m_start >= m_end; }
  std::span<OrcMark const> marks() const { return m_marks; }

  size_t size() const
  {
    if (m_start >= m_end)
      return 0;
    if (m_depth == 0)
      return 1;
    size_t s = mark_pos(m_marks, m_n_items, m_start);
    size_t e = mark_pos(m_marks,
                        m_n_items,
                        m_start + stride(m_marks,
                                         m_strides,
                                         m_stride_offset,
                                         m_start,
                                         static_cast<uint8_t>(m_depth - 1)));
    return e - s;
  }

  // Returns [start, end) item range for the current position.
  std::pair<size_t, size_t> range() const
  {
    if (m_depth == 0)
      return {m_start, m_start + 1};
    size_t s = mark_pos(m_marks, m_n_items, m_start);
    size_t e = mark_pos(m_marks,
                        m_n_items,
                        m_start + stride(m_marks,
                                         m_strides,
                                         m_stride_offset,
                                         m_start,
                                         static_cast<uint8_t>(m_depth - 1)));
    return {s, e};
  }

  ReadCursor child() const
  {
    if (m_depth == 0) {
      return ReadCursor(
        m_n_items, m_marks, m_strides, m_stride_offset, m_depth, m_start, m_end);
    }
    if (m_depth < 2) {
      size_t s = mark_pos(m_marks, m_n_items, m_start);
      size_t e = mark_pos(m_marks,
                          m_n_items,
                          m_start + stride(m_marks,
                                           m_strides,
                                           m_stride_offset,
                                           m_start,
                                           static_cast<uint8_t>(m_depth - 1)));
      return ReadCursor(m_n_items, m_marks, m_strides, m_stride_offset, 0, s, e);
    }
    size_t new_end =
      m_start +
      stride(
        m_marks, m_strides, m_stride_offset, m_start, static_cast<uint8_t>(m_depth - 1));
    return ReadCursor(m_n_items,
                      m_marks,
                      m_strides,
                      m_stride_offset,
                      static_cast<uint8_t>(m_depth - 1),
                      m_start,
                      new_end);
  }

  bool advance()
  {
    if (m_start >= m_end)
      return false;
    size_t new_start = m_start;
    if (m_depth == 0) {
      new_start += 1;
    }
    else {
      new_start += stride(
        m_marks, m_strides, m_stride_offset, m_start, static_cast<uint8_t>(m_depth - 1));
    }
    if (new_start < m_end) {
      m_start = new_start;
      return true;
    }
    return false;
  }
};

// ---------------------------------------------------------------------------
// WriteCursor
// ---------------------------------------------------------------------------

class WriteCursor
{
  uint8_t                m_depth;
  std::optional<uint8_t> m_next_depth;

public:
  WriteCursor(uint8_t depth, std::optional<uint8_t> next_depth)
      : m_depth(depth)
      , m_next_depth(next_depth)
  {}

  uint8_t                depth() const { return m_depth; }
  std::optional<uint8_t> next_depth() const { return m_next_depth; }

  std::optional<uint8_t> take_next_depth()
  {
    std::optional<uint8_t> result = m_next_depth;
    m_next_depth.reset();
    return result;
  }

  void advance() { m_next_depth = m_depth; }

  WriteCursor child()
  {
    uint8_t d = m_depth > 0 ? static_cast<uint8_t>(m_depth - 1) : uint8_t {0};
    std::optional<uint8_t> nd =
      m_next_depth.has_value() ? m_next_depth : std::optional<uint8_t> {d};
    m_next_depth.reset();
    return WriteCursor(d, nd);
  }

  WriteCursor take()
  {
    std::optional<uint8_t> nd =
      m_next_depth.has_value() ? m_next_depth : std::optional<uint8_t> {m_depth};
    m_next_depth.reset();
    return WriteCursor(m_depth, nd);
  }

  template<typename T>
  DeckWriter<T> writer(Deck<T> &deck);
};

// ---------------------------------------------------------------------------
// Deck
// ---------------------------------------------------------------------------

template<typename T>
class Deck
{
  template<typename U>
  friend class DeckWriter;

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
        dst           = std::min(dst, static_cast<uint64_t>(m_marks.size() - peg));
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

  size_t size() const { return m_items.size(); }

  bool empty() const { return m_items.empty(); }

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
    size_t               count     = m_marks.size();
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

  // Declared here, defined after DeckView/DeckWriter are complete.
  DeckView<T>   view(uint8_t depth) const;
  DeckWriter<T> writer(uint8_t depth);

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

// ---------------------------------------------------------------------------
// DeckView
// ---------------------------------------------------------------------------

template<typename T>
class DeckView
{
  std::span<T const> m_items;
  ReadCursor         m_cursor;

public:
  DeckView(std::span<T const> items, ReadCursor cursor)
      : m_items(items)
      , m_cursor(cursor)
  {}

  uint8_t depth() const { return m_cursor.depth(); }

  size_t size() const { return m_cursor.size(); }

  bool empty() const { return m_cursor.empty(); }

  T const &as_ref() const
  {
    if (m_cursor.depth() == 0)
      return m_items[m_cursor.start()];
    return m_items[static_cast<size_t>(m_cursor.marks()[m_cursor.start()].pos)];
  }

  std::span<T const> as_slice() const
  {
    std::pair<size_t, size_t> r = m_cursor.range();
    return m_items.subspan(r.first, r.second - r.first);
  }

  DeckView child() const { return DeckView(m_items, m_cursor.child()); }

  bool advance() { return m_cursor.advance(); }

  T const &operator[](size_t i) const { return as_slice()[i]; }

  std::span<T const>       items() const { return m_items; }
  std::span<OrcMark const> marks() const { return m_cursor.marks(); }
};

// ---------------------------------------------------------------------------
// DeckWriter
// ---------------------------------------------------------------------------

template<typename T>
class DeckWriter
{
  Deck<T>    *m_deck;
  WriteCursor m_cursor;
  size_t      m_start;

public:
  DeckWriter(Deck<T> *deck, WriteCursor cursor, size_t start)
      : m_deck(deck)
      , m_cursor(cursor)
      , m_start(start)
  {}

  ~DeckWriter()
  {
    if (m_deck == nullptr)
      return;
    std::optional<uint8_t> d = m_cursor.take_next_depth();
    if (d.has_value())
      m_deck->start_new_arr(d.value());
  }

  // Non-copyable.
  DeckWriter(DeckWriter const &)            = delete;
  DeckWriter &operator=(DeckWriter const &) = delete;

  // Movable.
  DeckWriter(DeckWriter &&other) noexcept
      : m_deck(other.m_deck)
      , m_cursor(other.m_cursor)
      , m_start(other.m_start)
  {
    other.m_deck = nullptr;
  }
  DeckWriter &operator=(DeckWriter &&other) noexcept
  {
    if (this != &other) {
      m_deck       = other.m_deck;
      m_cursor     = other.m_cursor;
      m_start      = other.m_start;
      other.m_deck = nullptr;
    }
    return *this;
  }

  DeckWriter child()
  {
    size_t start = m_deck->size();
    return DeckWriter(m_deck, m_cursor.child(), start);
  }

  uint8_t depth() const { return m_cursor.depth(); }

  void push(T item)
  {
    m_deck->push(std::move(item), m_cursor.take_next_depth().value_or(0));
  }

  void extend_from_slice(std::span<T const> items)
  {
    uint8_t d = m_cursor.take_next_depth().value_or(0);
    m_deck->start_new_arr(d);
    m_deck->m_items.insert(m_deck->m_items.end(), items.begin(), items.end());
  }

  size_t size() const { return m_deck->size() - m_start; }

  bool empty() const { return size() == 0; }

  std::span<T const> as_slice() const
  {
    return std::span<T const>(m_deck->m_items).subspan(m_start);
  }

  std::span<T> as_slice_mut() { return std::span<T>(m_deck->m_items).subspan(m_start); }

  T &push_default_mut()
  {
    size_t i = m_deck->m_items.size();
    push(T {});
    return m_deck->m_items[i];
  }

  std::span<T> push_default_mut_many(size_t n_items)
  {
    size_t start = m_deck->m_items.size();
    for (size_t i = 0; i < n_items; ++i)
      push(T {});
    return std::span<T>(m_deck->m_items).subspan(start);
  }

  T const &operator[](size_t i) const { return as_slice()[i]; }
  T       &operator[](size_t i) { return as_slice_mut()[i]; }
};

// ---------------------------------------------------------------------------
// Deck::view() and Deck::writer() definitions
// ---------------------------------------------------------------------------

template<typename T>
DeckView<T> Deck<T>::view(uint8_t depth) const
{
  size_t end = (depth == 0) ? m_items.size() : m_marks.size();
  return DeckView<T>(
    m_items,
    ReadCursor(m_items.size(), m_marks, m_strides, m_stride_offset, depth, 0, end));
}

template<typename T>
DeckWriter<T> Deck<T>::writer(uint8_t depth)
{
  size_t start = size();
  return DeckWriter<T>(this, WriteCursor(depth, depth), start);
}

// ---------------------------------------------------------------------------
// Display
// ---------------------------------------------------------------------------

template<typename T>
void fmt_raw_deck(std::span<T const>       items,
                  std::span<OrcMark const> marks,
                  std::ostream            &os)
{
  if (items.empty() && marks.empty()) {
    os << "<empty_deck>\n";
    return;
  }
  constexpr size_t TAB        = 3;
  uint8_t          dmax       = marks.empty() ? uint8_t {0} : marks.front().depth;
  uint64_t         n_items    = static_cast<uint64_t>(items.size());
  uint64_t         tail_start = 0;

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

// ---------------------------------------------------------------------------
// WriteCursor::writer() definition (deferred — needs Deck + DeckWriter)
// ---------------------------------------------------------------------------

template<typename T>
DeckWriter<T> WriteCursor::writer(Deck<T> &deck)
{
  size_t      start  = deck.size();
  WriteCursor cursor = take();
  return DeckWriter<T>(&deck, cursor, start);
}

// ---------------------------------------------------------------------------
// update_handle_from_deck
// ---------------------------------------------------------------------------

template<typename T>
void update_handle_from_deck(Deck<T> const &deck, OrcHandle &handle)
{
  handle.items         = deck.items().data();
  handle.n_items       = static_cast<uint64_t>(deck.items().size());
  handle.item_size     = static_cast<uint64_t>(sizeof(T));
  handle.marks         = deck.marks().data();
  handle.n_marks       = static_cast<uint64_t>(deck.marks().size());
  handle.stride_offset = deck.stride_offset().data();
  handle.strides       = deck.strides().data();
}

// ---------------------------------------------------------------------------
// Combinations
// ---------------------------------------------------------------------------

class Combinations
{
  std::vector<ReadCursor>  m_input_cursors;
  std::vector<uint8_t>     m_input_depths;
  std::vector<WriteCursor> m_output_cursors;
  std::vector<uint8_t>     m_output_depths;
  size_t                   m_stack_depth;

public:
  static std::pair<Combinations, Error> from_handles(
    std::span<OrcHandle const> inputs,
    std::span<uint8_t const>   input_depths,
    std::span<uint8_t const>   output_depths)
  {
    if (input_depths.size() != inputs.size())
      return {Combinations {}, Error::INVALID_COMBINATIONS};

    size_t n_outputs = output_depths.size();

    // Compute max_delta across all inputs.
    uint8_t max_delta = 0;
    for (size_t i = 0; i < inputs.size(); ++i) {
      std::span<OrcMark const> marks(inputs[i].marks,
                                     static_cast<size_t>(inputs[i].n_marks));
      uint8_t                  depth =
        marks.empty() ? uint8_t {0} : static_cast<uint8_t>(marks[0].depth + 1);
      uint8_t delta = depth > input_depths[i]
                        ? static_cast<uint8_t>(depth - input_depths[i])
                        : uint8_t {0};
      max_delta     = std::max(max_delta, delta);
    }
    size_t stack_depth = static_cast<size_t>(max_delta) + 1;

    // Build input cursors: for each input, telescope stack_depth cursors.
    std::vector<ReadCursor> input_cursors;
    input_cursors.reserve(inputs.size() * stack_depth);
    for (size_t i = 0; i < inputs.size(); ++i) {
      OrcHandle const          &input = inputs[i];
      std::span<OrcMark const>  marks(input.marks, static_cast<size_t>(input.n_marks));
      std::span<uint64_t const> stride_offset(input.stride_offset,
                                              static_cast<size_t>(input.n_marks));
      std::span<uint64_t const> strides(input.strides,
                                        calc_stride_count(marks, stride_offset));

      uint8_t depth =
        marks.empty() ? uint8_t {0} : static_cast<uint8_t>(marks[0].depth + 1);
      size_t end = (depth == 0) ? static_cast<size_t>(input.n_items)
                                : static_cast<size_t>(input.n_marks);

      ReadCursor prev(static_cast<size_t>(input.n_items),
                      marks,
                      strides,
                      stride_offset,
                      static_cast<uint8_t>(input_depths[i] + max_delta),
                      0,
                      end);
      for (size_t d = 0; d < stack_depth; ++d) {
        input_cursors.push_back(prev);
        prev = prev.child();
      }
    }

    // Build output cursors: for each output, telescope stack_depth cursors.
    std::vector<WriteCursor> output_cursors;
    output_cursors.reserve(n_outputs * stack_depth);
    for (size_t i = 0; i < n_outputs; ++i) {
      uint8_t depth = static_cast<uint8_t>(output_depths[i] + max_delta);
      output_cursors.push_back(WriteCursor(depth, depth));
      for (size_t d = 1; d < stack_depth; ++d) {
        WriteCursor child_cursor = output_cursors.back().child();
        output_cursors.push_back(child_cursor);
      }
    }

    Combinations comb;
    comb.m_input_cursors = std::move(input_cursors);
    comb.m_input_depths  = std::vector<uint8_t>(input_depths.begin(), input_depths.end());
    comb.m_output_cursors = std::move(output_cursors);
    comb.m_output_depths =
      std::vector<uint8_t>(output_depths.begin(), output_depths.end());
    comb.m_stack_depth = stack_depth;
    return {std::move(comb), Error::NONE};
  }

  bool advance()
  {
    // First try to advance the innermost inputs.
    bool any_advanced = false;
    for (size_t i = m_stack_depth - 1; i < m_input_cursors.size(); i += m_stack_depth) {
      bool advanced = m_input_cursors[i].advance();
      any_advanced  = any_advanced || advanced;
    }
    if (any_advanced) {
      for (size_t i = m_stack_depth - 1; i < m_output_cursors.size();
           i += m_stack_depth) {
        m_output_cursors[i].advance();
      }
      return true;
    }

    // None of the innermost inputs advanced. Walk up the stack.
    enum class State
    {
      CONTINUE,
      ADVANCED,
      EXHAUSTED
    };
    State  state     = State::CONTINUE;
    size_t stack_top = m_stack_depth - 1;
    while (true) {
      if (stack_top == 0) {
        state = State::EXHAUSTED;
        break;
      }
      stack_top -= 1;
      // Try to advance at this level.
      for (size_t i = stack_top; i < m_input_cursors.size(); i += m_stack_depth) {
        if (m_input_cursors[i].advance()) {
          state = State::ADVANCED;
        }
      }
      if (state == State::CONTINUE)
        continue;
      if (state == State::ADVANCED) {
        for (size_t i = stack_top; i < m_output_cursors.size(); i += m_stack_depth) {
          m_output_cursors[i].advance();
        }
      }
      break;
    }

    if (state == State::ADVANCED) {
      // Re-telescope inputs below stack_top.
      for (size_t i = 0; i < m_input_depths.size(); ++i) {
        size_t base = i * m_stack_depth;
        for (size_t d = stack_top + 1; d < m_stack_depth; ++d) {
          m_input_cursors[base + d] = m_input_cursors[base + d - 1].child();
        }
      }
      // Re-telescope outputs below stack_top.
      for (size_t i = 0; i < m_output_depths.size(); ++i) {
        size_t base = i * m_stack_depth;
        for (size_t d = stack_top + 1; d < m_stack_depth; ++d) {
          m_output_cursors[base + d] = m_output_cursors[base + d - 1].child();
        }
      }
      return true;
    }
    return false;
  }

  template<typename T>
  DeckView<T> get_input(std::span<T const> items, size_t index) const
  {
    ReadCursor cursor = m_input_cursors[(index + 1) * m_stack_depth - 1];
    return DeckView<T>(items, cursor);
  }

  template<typename T>
  DeckWriter<T> get_output(Deck<T> &deck, size_t index)
  {
    return m_output_cursors[(index + 1) * m_stack_depth - 1].writer(deck);
  }

private:
  Combinations() = default;
};

}  // namespace orc_sdk
