#include "error.hpp"

namespace orc_sdk {

std::ostream &operator<<(std::ostream &os, Error const e)
{
  switch (e) {
  case Error::NONE:
    break;
  case Error::ABI_VERSION_MISMATCH:
    os << "Abi version mismatch";
    break;
  case Error::INVALID_HANDLE:
    os << "Invalid handle";
    break;
  case Error::INVALID_DIMENSIONS:
    os << "Invalid dimensions";
    break;
  case Error::TYPE_MISMATCH:
    os << "Deck type mismatch";
    break;
  case Error::INVALID_COMBINATIONS:
    os << "Invalid combinations";
    break;
  case Error::PLUGIN_ALREADY_INITIALIZED:
    os << "Plugin already initialized";
    break;
  case Error::CONCURRENCY_PROBLEM:
    os << "Concurrency problem";
    break;
  case Error::INVALID_PROXY:
    os << "Invalid proxy";
    break;
  case Error::CANNOT_LOAD_PLUGINS:
    os << "Cannot load plugins";
    break;
  case Error::OUT_OF_BOUNDS:
    os << "Memory out of bounds";
    break;
  case Error::ALLOC_FAILED:
    os << "Memory allocation failed";
    break;
  case Error::NULL_PTR:
    os << "Null pointer";
    break;
  case Error::MISSING_CAPABILITY:
    os << "This capability is missing / not implemented";
    break;
  case Error::INVALID_FUNCTION:
    os << "The function is not valid, or not properly loaded from a plugin.";
    break;
  case Error::INVALID_ARGUMENTS:
    os << "The input/output arguments to this function are not valid.";
    break;
  case Error::SERIALIZATION_ERROR:
    os << "Serialization failed";
    break;
  case Error::INVALID_CONTEXT:
    os << "Invalid context used in a callback.";
    break;
  case Error::DECK_DEPTH_OVERFLOW:
    os << "The depth of the deck exceeds maximum supported depth";
    break;
  case Error::UNKNOWN:
    os << "Unknown error";
    break;
  }
  return os;
}

}  // namespace orc_sdk
