#pragma once

extern "C"
{
#include "orc_abi.h"
}
#include <cstddef>
#include <cstdint>
#include <vector>

namespace orc_sdk {

template<typename T>
class Deck
{
  std::vector<T>        items;
  std::vector<OrcMark>  marks;
  std::vector<uint64_t> stride_offset;
  std::vector<uint64_t> strides;
  std::vector<size_t>   pegs;
};

}  // namespace orc_sdk
