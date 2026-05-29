#pragma once

typedef enum
{
  OK = 0,
  ALLOC_FAILED,
  OUT_OF_BOUNDS,
} Status;

int stat_printf(Status const s);
