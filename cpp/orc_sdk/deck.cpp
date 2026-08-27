#include "deck.hpp"

#include <span>

namespace orc_sdk {

// ---------------------------------------------------------------------------
// calc_stride_count / calc_strides
// ---------------------------------------------------------------------------

size_t calc_stride_count(std::vector<OrcMark> const  &marks,
                         std::vector<uint64_t> const &stride_offset)
{
  if (marks.empty())
    return 0;
  return static_cast<size_t>(stride_offset.back()) + marks.back().depth;
}

size_t calc_stride_count(std::span<OrcMark const>  marks,
                         std::span<uint64_t const> stride_offset)
{
  if (marks.empty())
    return 0;
  return static_cast<size_t>(stride_offset.back()) + marks.back().depth;
}

void calc_strides(std::vector<OrcMark> const &marks,
                  std::vector<size_t>        &pegs,
                  std::vector<uint64_t>      &stride_offset,
                  std::vector<uint64_t>      &strides)
{
  pegs.clear();
  stride_offset.clear();
  strides.clear();
  // Exclusive prefix sum of depth.
  uint64_t acc = 0;
  for (OrcMark const &m : marks) {
    stride_offset.push_back(acc);
    acc += m.depth;
  }
  // One stride entry per depth level per mark.
  size_t total = calc_stride_count(marks, stride_offset);
  strides.assign(total, UINT64_MAX);
  // Fill strides using pegs.
  for (size_t i = 0; i < marks.size(); ++i) {
    size_t d = static_cast<size_t>(marks[i].depth);
    if (d > pegs.size())
      pegs.resize(d, 0);
    for (size_t j = 0; j < d; ++j) {
      size_t peg = pegs[j];
      if (peg < i) {
        uint64_t &dst = strides[static_cast<size_t>(stride_offset[peg]) + j];
        dst           = std::min(dst, static_cast<uint64_t>(i - peg));
      }
      pegs[j] = i;
    }
  }
}

// ---------------------------------------------------------------------------
// Free helper functions
// ---------------------------------------------------------------------------

size_t stride(std::span<OrcMark const>  marks,
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

size_t mark_pos(std::span<OrcMark const> marks, size_t n_items, size_t idx)
{
  if (idx < marks.size())
    return static_cast<size_t>(marks[idx].pos);
  return n_items;
}

// ---------------------------------------------------------------------------
// ReadCursor
// ---------------------------------------------------------------------------

ReadCursor::ReadCursor(size_t                    n_items,
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

size_t ReadCursor::size() const
{
  if (m_start >= m_end)
    return 0;
  if (m_depth == 0)
    return 1;
  size_t s = mark_pos(m_marks, m_n_items, m_start);
  size_t e = mark_pos(
    m_marks,
    m_n_items,
    m_start +
      stride(
        m_marks, m_strides, m_stride_offset, m_start, static_cast<uint8_t>(m_depth - 1)));
  return e - s;
}

std::pair<size_t, size_t> ReadCursor::range() const
{
  if (m_depth == 0)
    return {m_start, m_start + 1};
  size_t s = mark_pos(m_marks, m_n_items, m_start);
  size_t e = mark_pos(
    m_marks,
    m_n_items,
    m_start +
      stride(
        m_marks, m_strides, m_stride_offset, m_start, static_cast<uint8_t>(m_depth - 1)));
  return {s, e};
}

ReadCursor ReadCursor::child() const
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

bool ReadCursor::advance()
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

// ---------------------------------------------------------------------------
// WriteCursor
// ---------------------------------------------------------------------------

WriteCursor::WriteCursor(uint8_t depth, std::optional<uint8_t> next_depth)
    : m_depth(depth)
    , m_next_depth(next_depth)
{}

std::optional<uint8_t> WriteCursor::take_next_depth()
{
  std::optional<uint8_t> result = m_next_depth;
  m_next_depth.reset();
  return result;
}

void WriteCursor::advance()
{
  m_next_depth = m_depth;
}

WriteCursor WriteCursor::child()
{
  uint8_t d = m_depth > 0 ? static_cast<uint8_t>(m_depth - 1) : uint8_t {0};
  std::optional<uint8_t> nd =
    m_next_depth.has_value() ? m_next_depth : std::optional<uint8_t> {d};
  m_next_depth.reset();
  return WriteCursor(d, nd);
}

WriteCursor WriteCursor::take()
{
  std::optional<uint8_t> nd =
    m_next_depth.has_value() ? m_next_depth : std::optional<uint8_t> {m_depth};
  m_next_depth.reset();
  return WriteCursor(m_depth, nd);
}

// ---------------------------------------------------------------------------
// Combinations
// ---------------------------------------------------------------------------

std::pair<Combinations, Error> Combinations::from_handles(
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
  comb.m_input_cursors  = std::move(input_cursors);
  comb.m_input_depths   = std::vector<uint8_t>(input_depths.begin(), input_depths.end());
  comb.m_output_cursors = std::move(output_cursors);
  comb.m_output_depths = std::vector<uint8_t>(output_depths.begin(), output_depths.end());
  comb.m_stack_depth   = stack_depth;
  return {std::move(comb), Error::NONE};
}

bool Combinations::advance()
{
  // First try to advance the innermost inputs.
  bool any_advanced = false;
  for (size_t i = m_stack_depth - 1; i < m_input_cursors.size(); i += m_stack_depth) {
    bool advanced = m_input_cursors[i].advance();
    any_advanced  = any_advanced || advanced;
  }
  if (any_advanced) {
    for (size_t i = m_stack_depth - 1; i < m_output_cursors.size(); i += m_stack_depth) {
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

}  // namespace orc_sdk
