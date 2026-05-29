#include <stdio.h>

#include "status.h"

int stat_printf(Status const s)
{
  switch (s) {
  case OK:
    return printf("[OK]");
  case ALLOC_FAILED:
    return printf("[ERROR] Failed to allocate memory");
  case OUT_OF_BOUNDS:
    return printf("[ERROR] Index is out of bounds");
  default:  // Weird.
    return -1;
  }
}
