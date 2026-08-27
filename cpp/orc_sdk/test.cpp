extern "C"
{
#include <third_party/unity.h>
}

#include "orc_sdk/error.hpp"

#include <sstream>

void setUp(void) {}
void tearDown(void) {}

int main(void)
{
  UNITY_BEGIN();
  return UNITY_END();
}
