#include "deck.hpp"

#include <span>

namespace orc_sdk {

size_t calc_stride_count(std::vector<OrcMark> const  &marks,
                         std::vector<uint64_t> const &stride_offset)
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
        dst       = std::min(dst, static_cast<uint64_t>(i - peg));
      }
      pegs[j] = i;
    }
  }
}

size_t calc_stride_count(std::span<OrcMark const>  marks,
                         std::span<uint64_t const> stride_offset)
{
  if (marks.empty())
    return 0;
  return static_cast<size_t>(stride_offset.back()) + marks.back().depth;
}

}  // namespace orc_sdk
