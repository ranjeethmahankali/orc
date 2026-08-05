#include "orc_sdk.h"
#include "unity.h"

#include <limits.h>
#include <stddef.h>
#include <stdio.h>
#include <string.h>

#include <threads.h>  // C11 standard threads (same API as tinycthread)

#ifdef _MSC_VER
#include <intrin.h>
static int __builtin_ctzll(unsigned long long x)
{
  unsigned long index;
  _BitScanForward64(&index, x);
  return (int)index;
}
#endif

// The purpose of this struct is to check for maximum alignment compatibility of other
// types.
typedef union
{
  long long   ll;
  long double ld;
  void       *p;
} _MaxAlignCompat;

void test_arr_null_pointer_operations(void)
{
  double *null_arr = NULL;
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_arr_len(null_arr) == 0,
                           "Null pointer represents an empty array");
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_arr_end(null_arr) == null_arr,
                           "End of a NULL is itself");
  TEST_ASSERT_TRUE_MESSAGE(
    orc_sdk_arr_swap_remove(null_arr, 0) == ORC_ERROR_OUT_OF_BOUNDS,
    "Cannot remove from empty array");
  double *arr = NULL;
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_arr_reserve(arr, 10) == ORC_ERROR_NONE,
                           "Reserve starting with NULL");
  TEST_ASSERT_TRUE_MESSAGE(arr != NULL, "Should be allocated after reserve");
  orc_sdk_arr_free(arr);
  // Should not crash
  double *ptr = NULL;
  orc_sdk_arr_free(ptr);
}

void test_arr_empty_array_operations(void)
{
  double *arr = NULL;
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_arr_reserve(arr, 0) == ORC_ERROR_NONE,
                           "Empty array reserve");
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_arr_len(arr) == 0, "Length after reserved is zero");
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_arr_swap_remove(arr, 0) == ORC_ERROR_OUT_OF_BOUNDS,
                           "Cannot remove from empty array");
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_arr_push(arr, 1.0) == ORC_ERROR_NONE,
                           "Push into empty array");
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_arr_len(arr) == 1, "Array after pushing");
  orc_sdk_arr_free(arr);
}

void test_arr_index_boundary_conditions(void)
{
  double *arr = NULL;
  orc_sdk_arr_push(arr, 1.0);
  // Single element array
  TEST_ASSERT_TRUE(orc_sdk_arr_swap_remove(arr, 0) == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(orc_sdk_arr_len(arr) == 0);
  TEST_ASSERT_TRUE(orc_sdk_arr_swap_remove(arr, 0) ==
                   ORC_ERROR_OUT_OF_BOUNDS);  // Empty array
  // Add elements back
  orc_sdk_arr_push(arr, 1.0);
  orc_sdk_arr_push(arr, 2.0);
  TEST_ASSERT_TRUE(orc_sdk_arr_swap_remove(arr, orc_sdk_arr_len(arr)) ==
                   ORC_ERROR_OUT_OF_BOUNDS);  // One past end
  TEST_ASSERT_TRUE(orc_sdk_arr_swap_remove(arr, orc_sdk_arr_len(arr) + 10) ==
                   ORC_ERROR_OUT_OF_BOUNDS);  // Way past end
  TEST_ASSERT_TRUE(orc_sdk_arr_swap_remove(arr, SIZE_MAX) ==
                   ORC_ERROR_OUT_OF_BOUNDS);  // Huge index
  orc_sdk_arr_free(arr);
}

void test_orc_sdk_arr_capacity_management(void)
{
  double *arr = NULL;
  // Reserve initial capacity
  TEST_ASSERT_TRUE(orc_sdk_arr_reserve(arr, 4) == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(_orc_sdk_arr_capacity(arr) == 4);
  // Reserve smaller (should be no-op)
  size_t old_cap = _orc_sdk_arr_capacity(arr);
  TEST_ASSERT_TRUE(orc_sdk_arr_reserve(arr, 2) == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(_orc_sdk_arr_capacity(arr) == old_cap);
  // Reserve exact current capacity (should be no-op)
  TEST_ASSERT_TRUE(orc_sdk_arr_reserve(arr, old_cap) == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(_orc_sdk_arr_capacity(arr) == old_cap);
  // Reserve larger
  TEST_ASSERT_TRUE(orc_sdk_arr_reserve(arr, 10) == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(_orc_sdk_arr_capacity(arr) == 10);
  // Test growth pattern
  orc_sdk_arr_free(arr);
  arr = NULL;
  TEST_ASSERT_TRUE(orc_sdk_arr_push(arr, 1.0) == ORC_ERROR_NONE);
  size_t cap1 = _orc_sdk_arr_capacity(arr);
  // Fill to capacity
  while (orc_sdk_arr_len(arr) < cap1) {
    orc_sdk_arr_push(arr, (double)orc_sdk_arr_len(arr));
  }
  // Next push should grow
  orc_sdk_arr_push(arr, 999.0);
  TEST_ASSERT_TRUE(_orc_sdk_arr_capacity(arr) > cap1);
  orc_sdk_arr_free(arr);
}

void test_arr_double_free_safety(void)
{
  double *arr = NULL;
  orc_sdk_arr_push(arr, 1.0);
  orc_sdk_arr_free(arr);  // First free, arr becomes NULL
  orc_sdk_arr_free(arr);  // Second free on NULL, should not crash
}

void test_arr_swap_remove_correctness(void)
{
  double *arr = NULL;
  // Setup: [0, 1, 2, 3, 4]
  for (int i = 0; i < 5; i++) {
    orc_sdk_arr_push(arr, (double)i);
  }
  // Remove middle element (index 2, value 2.0)
  // Should replace with last element (value 4.0)
  TEST_ASSERT_TRUE(orc_sdk_arr_swap_remove(arr, 2) == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(orc_sdk_arr_len(arr) == 4);
  TEST_ASSERT_TRUE(arr[2] == 4.0);  // Last element moved here
  // Remove first element
  TEST_ASSERT_TRUE(orc_sdk_arr_swap_remove(arr, 0) == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(orc_sdk_arr_len(arr) == 3);
  TEST_ASSERT_TRUE(arr[0] == 3.0);  // Last element moved to front
  // Remove last element
  size_t last_idx = orc_sdk_arr_len(arr) - 1;
  TEST_ASSERT_TRUE(orc_sdk_arr_swap_remove(arr, last_idx) == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(orc_sdk_arr_len(arr) == 2);
  // Remove from single-element array
  orc_sdk_arr_swap_remove(arr, 0);
  orc_sdk_arr_swap_remove(arr, 0);
  TEST_ASSERT_TRUE(orc_sdk_arr_len(arr) == 0);
  orc_sdk_arr_free(arr);
}

void test_arr_memory_stress(void)
{
  double      *arr        = NULL;
  const size_t LARGE_SIZE = 1000;
  // Push many elements
  for (size_t i = 0; i < LARGE_SIZE; i++) {
    TEST_ASSERT_TRUE(orc_sdk_arr_push(arr, (double)i) == ORC_ERROR_NONE);
  }
  // Verify all elements
  TEST_ASSERT_TRUE(orc_sdk_arr_len(arr) == LARGE_SIZE);
  for (size_t i = 0; i < LARGE_SIZE; i++) {
    TEST_ASSERT_TRUE(arr[i] == (double)i);
  }
  // Repeated push/remove cycles
  for (int cycle = 0; cycle < 100; cycle++) {
    size_t old_len = orc_sdk_arr_len(arr);
    orc_sdk_arr_push(arr, 999.0);
    TEST_ASSERT_TRUE(orc_sdk_arr_len(arr) == old_len + 1);
    orc_sdk_arr_swap_remove(arr, orc_sdk_arr_len(arr) - 1);
    TEST_ASSERT_TRUE(orc_sdk_arr_len(arr) == old_len);
  }
  orc_sdk_arr_free(arr);
}

void test_arr_different_types(void)
{
  // Test with int
  int *ints = NULL;
  TEST_ASSERT_TRUE(orc_sdk_arr_push(ints, 42) == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(orc_sdk_arr_push(ints, -17) == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(orc_sdk_arr_len(ints) == 2);
  TEST_ASSERT_TRUE(ints[0] == 42);
  TEST_ASSERT_TRUE(ints[1] == -17);
  orc_sdk_arr_free(ints);
  // Test with char
  char *chars = NULL;
  TEST_ASSERT_TRUE(orc_sdk_arr_push(chars, 'A') == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(orc_sdk_arr_push(chars, 'B') == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(chars[0] == 'A');
  TEST_ASSERT_TRUE(chars[1] == 'B');
  orc_sdk_arr_free(chars);
  // Test with pointers
  const char  *strings[] = {"hello", "world"};
  const char **str_ptrs  = NULL;
  TEST_ASSERT_TRUE(orc_sdk_arr_push(str_ptrs, strings[0]) == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(orc_sdk_arr_push(str_ptrs, strings[1]) == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(str_ptrs[0] == strings[0]);
  TEST_ASSERT_TRUE(str_ptrs[1] == strings[1]);
  orc_sdk_arr_free(str_ptrs);
  // Test with struct
  typedef struct
  {
    int    x, y;
    double value;
  } Point;
  Point *points = NULL;
  Point  p1     = {10, 20, 3.14};
  Point  p2     = {-5, 15, 2.71};
  TEST_ASSERT_TRUE(orc_sdk_arr_push(points, p1) == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(orc_sdk_arr_push(points, p2) == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(points[0].x == 10);
  TEST_ASSERT_TRUE(points[0].y == 20);
  TEST_ASSERT_TRUE(points[0].value == 3.14);
  TEST_ASSERT_TRUE(points[1].x == -5);
  TEST_ASSERT_TRUE(points[1].y == 15);
  TEST_ASSERT_TRUE(points[1].value == 2.71);
  orc_sdk_arr_free(points);
  // Test alignment by checking data pointer alignment
  long long *longs = NULL;
  TEST_ASSERT_TRUE(orc_sdk_arr_push(longs, 123456789LL) == ORC_ERROR_NONE);
  // Check that data pointer is properly aligned for long long
  uintptr_t addr = (uintptr_t)longs;
  TEST_ASSERT_TRUE_MESSAGE(addr % sizeof(long long) == 0,
                           "long long array not properly aligned");
  orc_sdk_arr_free(longs);
}

void test_arr_ordered_remove(void)
{
  int *arr = NULL;
  // Setup: [10, 20, 30, 40, 50]
  for (int i = 1; i <= 5; i++) {
    TEST_ASSERT_TRUE(orc_sdk_arr_push(arr, i * 10) == ORC_ERROR_NONE);
  }
  // Remove middle element (index 2, value 30)
  // Should shift [40, 50] left to fill the gap
  TEST_ASSERT_TRUE(orc_sdk_arr_remove(arr, 2) == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(orc_sdk_arr_len(arr) == 4);
  TEST_ASSERT_TRUE_MESSAGE(arr[0] == 10, "First element should be unchanged");
  TEST_ASSERT_TRUE_MESSAGE(arr[1] == 20, "Second element should be unchanged");
  TEST_ASSERT_TRUE_MESSAGE(arr[2] == 40, "Third element should be 40 (was 4th)");
  TEST_ASSERT_TRUE_MESSAGE(arr[3] == 50, "Fourth element should be 50 (was 5th)");
  // Remove first element
  // Should shift [20, 40, 50] left
  TEST_ASSERT_TRUE(orc_sdk_arr_remove(arr, 0) == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(orc_sdk_arr_len(arr) == 3);
  TEST_ASSERT_TRUE_MESSAGE(arr[0] == 20, "First element should now be 20");
  TEST_ASSERT_TRUE_MESSAGE(arr[1] == 40, "Second element should be 40");
  TEST_ASSERT_TRUE_MESSAGE(arr[2] == 50, "Third element should be 50");
  // Remove last element
  // Should just decrease count, no shifting needed
  TEST_ASSERT_TRUE(orc_sdk_arr_remove(arr, 2) == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(orc_sdk_arr_len(arr) == 2);
  TEST_ASSERT_TRUE_MESSAGE(arr[0] == 20, "First element unchanged");
  TEST_ASSERT_TRUE_MESSAGE(arr[1] == 40, "Second element unchanged");
  // Remove from single-element array
  orc_sdk_arr_remove(arr, 0);
  orc_sdk_arr_remove(arr, 0);
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_arr_len(arr) == 0, "Array should be empty");
  // Test bounds checking
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_arr_remove(arr, 0) == ORC_ERROR_OUT_OF_BOUNDS,
                           "Remove from empty array should fail");
  // Add one element and test invalid indices
  orc_sdk_arr_push(arr, 100);
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_arr_remove(arr, 1) == ORC_ERROR_OUT_OF_BOUNDS,
                           "Remove past end should fail");
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_arr_remove(arr, SIZE_MAX) == ORC_ERROR_OUT_OF_BOUNDS,
                           "Remove huge index should fail");
  orc_sdk_arr_free(arr);
  // Test with NULL pointer
  int *null_arr = NULL;
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_arr_remove(null_arr, 0) == ORC_ERROR_OUT_OF_BOUNDS,
                           "Remove from NULL should fail");
}

void test_arr_resize_zero_fill(void)
{
  double *arr = NULL;
  // Test resize from empty array - should zero-fill all elements
  orc_sdk_arr_resize(arr, 5);
  TEST_ASSERT_TRUE_MESSAGE(arr != NULL, "Resize from empty should succeed");
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_arr_len(arr) == 5, "Array should have 5 elements");
  for (size_t i = 0; i < 5; i++) {
    TEST_ASSERT_TRUE_MESSAGE(arr[i] == 0.0, "All elements should be zero-initialized");
  }
  // Fill array with known values to test growth behavior
  for (size_t i = 0; i < 5; i++) {
    arr[i] = (double)(i + 10);  // [10, 11, 12, 13, 14]
  }
  // Test resize to larger size (growth) - new elements should be zero
  orc_sdk_arr_resize(arr, 8);
  TEST_ASSERT_TRUE_MESSAGE(arr != NULL, "Resize growth should succeed");
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_arr_len(arr) == 8, "Array should have 8 elements");
  TEST_ASSERT_TRUE_MESSAGE(_orc_sdk_arr_capacity(arr) >= 8,
                           "Capacity should accommodate new size");
  // Check original elements unchanged
  for (size_t i = 0; i < 5; i++) {
    TEST_ASSERT_TRUE_MESSAGE(arr[i] == (double)(i + 10),
                             "Original elements should be unchanged");
  }
  // Check new elements are zero-filled
  for (size_t i = 5; i < 8; i++) {
    TEST_ASSERT_TRUE_MESSAGE(arr[i] == 0.0, "New elements should be zero-initialized");
  }
  // Test resize to smaller size (shrink) - remaining elements preserved
  orc_sdk_arr_resize(arr, 3);
  TEST_ASSERT_TRUE_MESSAGE(arr != NULL, "Resize shrink should succeed");
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_arr_len(arr) == 3, "Array should have 3 elements");
  // Capacity should remain the same (no reallocation on shrink)
  TEST_ASSERT_TRUE_MESSAGE(_orc_sdk_arr_capacity(arr) >= 8,
                           "Capacity should not decrease on shrink");
  // Check remaining elements unchanged
  for (size_t i = 0; i < 3; i++) {
    TEST_ASSERT_TRUE_MESSAGE(arr[i] == (double)(i + 10),
                             "Remaining elements should be unchanged");
  }
  // Test resize to same size (no-op)
  size_t len_before = orc_sdk_arr_len(arr);
  size_t cap_before = _orc_sdk_arr_capacity(arr);
  orc_sdk_arr_resize(arr, 3);
  TEST_ASSERT_TRUE_MESSAGE(arr != NULL, "Resize same size should succeed");
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_arr_len(arr) == len_before,
                           "Length should be unchanged");
  TEST_ASSERT_TRUE_MESSAGE(_orc_sdk_arr_capacity(arr) == cap_before,
                           "Capacity should be unchanged");
  for (size_t i = 0; i < 3; i++) {
    TEST_ASSERT_TRUE_MESSAGE(arr[i] == (double)(i + 10), "Elements should be unchanged");
  }
  // Test resize to zero (empty)
  orc_sdk_arr_resize(arr, 0);
  TEST_ASSERT_TRUE_MESSAGE(arr != NULL, "Resize to zero should succeed");
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_arr_len(arr) == 0, "Array should be empty");
  TEST_ASSERT_TRUE_MESSAGE(_orc_sdk_arr_capacity(arr) >= 8,
                           "Capacity should be preserved");
  // Test resize from zero back to non-zero - should zero-fill
  orc_sdk_arr_resize(arr, 4);
  TEST_ASSERT_TRUE_MESSAGE(arr != NULL, "Resize from zero should succeed");
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_arr_len(arr) == 4, "Array should have 4 elements");
  for (size_t i = 0; i < 4; i++) {
    TEST_ASSERT_TRUE_MESSAGE(arr[i] == 0.0, "Elements should be zero-initialized");
  }
  orc_sdk_arr_free(arr);
  // Test resize with NULL array - should create and zero-fill
  double *null_arr = NULL;
  orc_sdk_arr_resize(null_arr, 3);
  TEST_ASSERT_TRUE_MESSAGE(null_arr != NULL, "Resize NULL array should succeed");
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_arr_len(null_arr) == 3,
                           "Array should have 3 elements");
  for (size_t i = 0; i < 3; i++) {
    TEST_ASSERT_TRUE_MESSAGE(null_arr[i] == 0.0, "Elements should be zero-initialized");
  }
  orc_sdk_arr_free(null_arr);
  // Test with different types to ensure zero-initialization works correctly
  int *int_arr = NULL;
  orc_sdk_arr_resize(int_arr, 3);
  TEST_ASSERT_TRUE_MESSAGE(int_arr != NULL, "Int array resize should succeed");
  for (size_t i = 0; i < 3; i++) {
    TEST_ASSERT_TRUE_MESSAGE(int_arr[i] == 0, "Int elements should be zero-initialized");
  }
  orc_sdk_arr_free(int_arr);
  // Test with pointers
  void **ptr_arr = NULL;
  orc_sdk_arr_resize(ptr_arr, 2);
  TEST_ASSERT_TRUE_MESSAGE(ptr_arr != NULL, "Pointer array resize should succeed");
  for (size_t i = 0; i < 2; i++) {
    TEST_ASSERT_TRUE_MESSAGE(ptr_arr[i] == NULL,
                             "Pointer elements should be NULL-initialized");
  }
  orc_sdk_arr_free(ptr_arr);
  // Test large resize to verify performance and correctness
  double      *large_arr  = NULL;
  const size_t LARGE_SIZE = 10000;
  orc_sdk_arr_resize(large_arr, LARGE_SIZE);
  TEST_ASSERT_TRUE_MESSAGE(large_arr != NULL, "Large resize should succeed");
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_arr_len(large_arr) == LARGE_SIZE,
                           "Large array should have correct length");
  // Spot check zero-initialization (checking all would be slow)
  TEST_ASSERT_TRUE_MESSAGE(large_arr[0] == 0.0, "First element should be zero");
  TEST_ASSERT_TRUE_MESSAGE(large_arr[LARGE_SIZE / 2] == 0.0,
                           "Middle element should be zero");
  TEST_ASSERT_TRUE_MESSAGE(large_arr[LARGE_SIZE - 1] == 0.0,
                           "Last element should be zero");
  orc_sdk_arr_free(large_arr);
  // Test edge case: resize to 1 element
  double *single_arr = NULL;
  orc_sdk_arr_resize(single_arr, 1);
  TEST_ASSERT_TRUE_MESSAGE(single_arr != NULL, "Single element resize should succeed");
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_arr_len(single_arr) == 1,
                           "Array should have 1 element");
  TEST_ASSERT_TRUE_MESSAGE(single_arr[0] == 0.0,
                           "Single element should be zero-initialized");
  // Modify the element then resize larger
  single_arr[0] = 42.0;
  orc_sdk_arr_resize(single_arr, 3);
  TEST_ASSERT_TRUE_MESSAGE(single_arr[0] == 42.0, "Original element should be preserved");
  TEST_ASSERT_TRUE_MESSAGE(single_arr[1] == 0.0,
                           "New elements should be zero-initialized");
  TEST_ASSERT_TRUE_MESSAGE(single_arr[2] == 0.0,
                           "New elements should be zero-initialized");
  orc_sdk_arr_free(single_arr);
}

void test_orc_sdk_arr_fill(void)
{
  // Test 1: Basic fill with integers (power of 2 size)
  int *ints = NULL;
  orc_sdk_arr_resize(ints, 4);
  int val = 42;
  orc_sdk_arr_fill(ints, val);
  TEST_ASSERT_TRUE(orc_sdk_arr_len(ints) == 4);
  for (size_t i = 0; i < 4; i++) {
    TEST_ASSERT_TRUE_MESSAGE(ints[i] == 42, "All elements should be 42");
  }
  // Test 2: Non-power of 2 size
  orc_sdk_arr_resize(ints, 7);
  val = 77;
  orc_sdk_arr_fill(ints, val);
  TEST_ASSERT_TRUE(orc_sdk_arr_len(ints) == 7);
  for (size_t i = 0; i < 7; i++) {
    TEST_ASSERT_TRUE_MESSAGE(ints[i] == 77, "All elements should be 77");
  }
  // Test 3: Large size (to test doubling logic efficiency/correctness)
  orc_sdk_arr_resize(ints, 1025);
  val = 123;
  orc_sdk_arr_fill(ints, val);
  TEST_ASSERT_TRUE(orc_sdk_arr_len(ints) == 1025);
  for (size_t i = 0; i < 1025; i++) {
    TEST_ASSERT_TRUE_MESSAGE(ints[i] == 123, "All elements should be 123");
  }
  orc_sdk_arr_free(ints);
  // Test 4: Single element
  double *doubles = NULL;
  orc_sdk_arr_resize(doubles, 1);
  double dval = 3.14;
  orc_sdk_arr_fill(doubles, dval);
  TEST_ASSERT_TRUE(orc_sdk_arr_len(doubles) == 1);
  TEST_ASSERT_TRUE(doubles[0] == 3.14);
  orc_sdk_arr_free(doubles);
  // Test 5: Empty array
  float *floats = NULL;
  dval          = 1.0f;
  orc_sdk_arr_fill(floats, dval);  // orc_sdk_arr_len(NULL) is 0
  TEST_ASSERT_TRUE(orc_sdk_arr_len(floats) == 0);
  orc_sdk_arr_free(floats);
  // Test 6: Different types (Structs)
  typedef struct
  {
    int    a;
    double b;
  } TestStruct;
  TestStruct *structs = NULL;
  TestStruct  sval    = {10, 20.0};
  orc_sdk_arr_resize(structs, 3);
  orc_sdk_arr_fill(structs, sval);
  TEST_ASSERT_TRUE(orc_sdk_arr_len(structs) == 3);
  for (size_t i = 0; i < 3; i++) {
    TEST_ASSERT_TRUE(structs[i].a == 10);
    TEST_ASSERT_TRUE(structs[i].b == 20.0);
  }
  orc_sdk_arr_free(structs);
}

void test_orc_sdk_arr_clear(void)
{
  double *arr = NULL;
  // Test clear on empty array
  orc_sdk_arr_clear(arr);
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_arr_len(arr) == 0, "Clear on NULL array should work");
  // Add some elements
  orc_sdk_arr_resize(arr, 5);
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_arr_len(arr) == 5, "Array should have 5 elements");
  size_t old_capacity = _orc_sdk_arr_capacity(arr);
  // Clear the array
  orc_sdk_arr_clear(arr);
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_arr_len(arr) == 0,
                           "Array should be empty after clear");
  TEST_ASSERT_TRUE_MESSAGE(_orc_sdk_arr_capacity(arr) == old_capacity,
                           "Capacity should be preserved");
  // Verify we can still use the array after clear
  OrcError s = orc_sdk_arr_push(arr, 2.71);
  TEST_ASSERT_TRUE_MESSAGE(s == ORC_ERROR_NONE, "Should be able to push after clear");
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_arr_len(arr) == 1,
                           "Array should have 1 element after push");
  TEST_ASSERT_TRUE_MESSAGE(arr[0] == 2.71, "Element should be correct");
  // Clear again with elements
  orc_sdk_arr_push(arr, 1.0);
  orc_sdk_arr_push(arr, 2.0);
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_arr_len(arr) == 3, "Array should have 3 elements");
  orc_sdk_arr_clear(arr);
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_arr_len(arr) == 0,
                           "Array should be empty after second clear");
  // Test operations on cleared array
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_arr_swap_remove(arr, 0) == ORC_ERROR_OUT_OF_BOUNDS,
                           "Remove from cleared array should fail");
  orc_sdk_arr_free(arr);
  // Test clear on NULL pointer (should not crash)
  double *null_arr = NULL;
  orc_sdk_arr_clear(null_arr);  // Should not crash
}

void test_arr_remove_range(void)
{
  int *arr = NULL;
  // Setup test array: [10, 20, 30, 40, 50]
  for (int i = 1; i <= 5; i++) {
    OrcError s = orc_sdk_arr_push(arr, i * 10);
    TEST_ASSERT_TRUE_MESSAGE(s == ORC_ERROR_NONE, "Setup should succeed");
  }
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_arr_len(arr) == 5, "Array should have 5 elements");
  // Test 1: Remove middle range [1, 3) -> removes 20, 30
  // Expected: [10, 40, 50]
  OrcError result = orc_sdk_arr_remove_range(arr, 1, 3);
  TEST_ASSERT_TRUE_MESSAGE(result == ORC_ERROR_NONE,
                           "Remove middle range should succeed");
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_arr_len(arr) == 3,
                           "Array should have 3 elements after removing 2");
  TEST_ASSERT_TRUE_MESSAGE(arr[0] == 10, "First element should be unchanged");
  TEST_ASSERT_TRUE_MESSAGE(arr[1] == 40, "Second element should be 40 (was 4th)");
  TEST_ASSERT_TRUE_MESSAGE(arr[2] == 50, "Third element should be 50 (was 5th)");
  // Test 2: Remove from beginning [0, 1) -> removes 10
  // Expected: [40, 50]
  result = orc_sdk_arr_remove_range(arr, 0, 1);
  TEST_ASSERT_TRUE_MESSAGE(result == ORC_ERROR_NONE,
                           "Remove from beginning should succeed");
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_arr_len(arr) == 2, "Array should have 2 elements");
  TEST_ASSERT_TRUE_MESSAGE(arr[0] == 40, "First element should be 40");
  TEST_ASSERT_TRUE_MESSAGE(arr[1] == 50, "Second element should be 50");
  // Test 3: Remove from end [1, 2) -> removes 50
  // Expected: [40]
  result = orc_sdk_arr_remove_range(arr, 1, 2);
  TEST_ASSERT_TRUE_MESSAGE(result == ORC_ERROR_NONE, "Remove from end should succeed");
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_arr_len(arr) == 1, "Array should have 1 element");
  TEST_ASSERT_TRUE_MESSAGE(arr[0] == 40, "Remaining element should be 40");
  // Test 4: Remove entire array [0, 1)
  result = orc_sdk_arr_remove_range(arr, 0, 1);
  TEST_ASSERT_TRUE_MESSAGE(result == ORC_ERROR_NONE,
                           "Remove entire array should succeed");
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_arr_len(arr) == 0, "Array should be empty");
  // Test 5: Empty range operations
  // Add elements back
  orc_sdk_arr_push(arr, 100);
  orc_sdk_arr_push(arr, 200);
  orc_sdk_arr_push(arr, 300);
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_arr_len(arr) == 3, "Array should have 3 elements");
  // Remove empty range at beginning [0, 0)
  result = orc_sdk_arr_remove_range(arr, 0, 0);
  TEST_ASSERT_TRUE_MESSAGE(result == ORC_ERROR_NONE,
                           "Empty range at beginning should succeed");
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_arr_len(arr) == 3, "Array length should be unchanged");
  // Remove empty range in middle [1, 1)
  result = orc_sdk_arr_remove_range(arr, 1, 1);
  TEST_ASSERT_TRUE_MESSAGE(result == ORC_ERROR_NONE,
                           "Empty range in middle should succeed");
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_arr_len(arr) == 3, "Array length should be unchanged");
  // Remove empty range at end [3, 3)
  result = orc_sdk_arr_remove_range(arr, 3, 3);
  TEST_ASSERT_TRUE_MESSAGE(result == ORC_ERROR_NONE, "Empty range at end should succeed");
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_arr_len(arr) == 3, "Array length should be unchanged");
  // Test 6: Error cases - out of bounds
  // Start index too large
  result = orc_sdk_arr_remove_range(arr, 4, 4);
  TEST_ASSERT_TRUE_MESSAGE(result == ORC_ERROR_OUT_OF_BOUNDS,
                           "Start beyond array should fail");
  // Stop index too large
  result = orc_sdk_arr_remove_range(arr, 1, 5);
  TEST_ASSERT_TRUE_MESSAGE(result == ORC_ERROR_OUT_OF_BOUNDS,
                           "Stop beyond array should fail");
  // Invalid range (stop < start)
  result = orc_sdk_arr_remove_range(arr, 2, 1);
  TEST_ASSERT_TRUE_MESSAGE(result == ORC_ERROR_OUT_OF_BOUNDS,
                           "Invalid range should fail");
  // Test 7: Remove everything [0, length)
  result = orc_sdk_arr_remove_range(arr, 0, orc_sdk_arr_len(arr));
  TEST_ASSERT_TRUE_MESSAGE(result == ORC_ERROR_NONE,
                           "Remove all elements should succeed");
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_arr_len(arr) == 0, "Array should be empty");
  orc_sdk_arr_free(arr);
  // Test 8: Operations on NULL array
  int *null_arr = NULL;
  result        = orc_sdk_arr_remove_range(null_arr, 0, 0);
  TEST_ASSERT_TRUE_MESSAGE(result == ORC_ERROR_OUT_OF_BOUNDS,
                           "Remove from NULL array should fail");
  result = orc_sdk_arr_remove_range(null_arr, 0, 1);
  TEST_ASSERT_TRUE_MESSAGE(result == ORC_ERROR_OUT_OF_BOUNDS,
                           "Remove from NULL array should fail");
  // Test 9: Large range removal
  int *large_arr = NULL;
  for (int i = 0; i < 10; i++) {
    orc_sdk_arr_push(large_arr, i);
  }
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_arr_len(large_arr) == 10,
                           "Large array should have 10 elements");
  // Remove middle chunk [3, 7) -> removes 3, 4, 5, 6
  result = orc_sdk_arr_remove_range(large_arr, 3, 7);
  TEST_ASSERT_TRUE_MESSAGE(result == ORC_ERROR_NONE,
                           "Large range removal should succeed");
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_arr_len(large_arr) == 6,
                           "Array should have 6 elements remaining");
  // Verify elements: should be [0, 1, 2, 7, 8, 9]
  int expected[] = {0, 1, 2, 7, 8, 9};
  for (size_t i = 0; i < 6; i++) {
    TEST_ASSERT_TRUE_MESSAGE(large_arr[i] == expected[i],
                             "Large range removal elements should be correct");
  }
  orc_sdk_arr_free(large_arr);
}

void test_arr_pop(void)
{
  double *arr   = NULL;
  double  value = 0.0;
  // Test pop from empty array (should fail)
  OrcError result = orc_sdk_arr_pop(arr, &value);
  TEST_ASSERT_TRUE_MESSAGE(result == ORC_ERROR_OUT_OF_BOUNDS,
                           "Pop from empty array should fail");
  // Test pop from NULL array (should fail)
  double *null_arr = NULL;
  result           = orc_sdk_arr_pop(null_arr, &value);
  TEST_ASSERT_TRUE_MESSAGE(result == ORC_ERROR_OUT_OF_BOUNDS,
                           "Pop from NULL array should fail");
  // Setup array with known values: [10.0, 20.0, 30.0]
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_arr_push(arr, 10.0) == ORC_ERROR_NONE,
                           "Push should succeed");
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_arr_push(arr, 20.0) == ORC_ERROR_NONE,
                           "Push should succeed");
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_arr_push(arr, 30.0) == ORC_ERROR_NONE,
                           "Push should succeed");
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_arr_len(arr) == 3, "Array should have 3 elements");
  // Test pop from array with multiple elements
  result = orc_sdk_arr_pop(arr, &value);
  TEST_ASSERT_TRUE_MESSAGE(result == ORC_ERROR_NONE, "Pop should succeed");
  TEST_ASSERT_TRUE_MESSAGE(value == 30.0, "Popped value should be 30.0 (last element)");
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_arr_len(arr) == 2,
                           "Array should have 2 elements after pop");
  TEST_ASSERT_TRUE_MESSAGE(arr[0] == 10.0 && arr[1] == 20.0,
                           "Remaining elements should be correct");
  // Test sequential pops
  result = orc_sdk_arr_pop(arr, &value);
  TEST_ASSERT_TRUE_MESSAGE(result == ORC_ERROR_NONE, "Second pop should succeed");
  TEST_ASSERT_TRUE_MESSAGE(value == 20.0, "Popped value should be 20.0");
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_arr_len(arr) == 1,
                           "Array should have 1 element after second pop");
  TEST_ASSERT_TRUE_MESSAGE(arr[0] == 10.0, "Remaining element should be 10.0");
  // Test pop from single-element array
  result = orc_sdk_arr_pop(arr, &value);
  TEST_ASSERT_TRUE_MESSAGE(result == ORC_ERROR_NONE,
                           "Pop from single element should succeed");
  TEST_ASSERT_TRUE_MESSAGE(value == 10.0, "Popped value should be 10.0");
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_arr_len(arr) == 0,
                           "Array should be empty after popping last element");
  // Test pop from now-empty array (should fail)
  result = orc_sdk_arr_pop(arr, &value);
  TEST_ASSERT_TRUE_MESSAGE(result == ORC_ERROR_OUT_OF_BOUNDS,
                           "Pop from empty array should fail");
  orc_sdk_arr_free(arr);
  // Test with different data types
  int *int_arr   = NULL;
  int  int_value = 0;
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_arr_push(int_arr, 42) == ORC_ERROR_NONE,
                           "Int push should succeed");
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_arr_push(int_arr, 99) == ORC_ERROR_NONE,
                           "Int push should succeed");
  result = orc_sdk_arr_pop(int_arr, &int_value);
  TEST_ASSERT_TRUE_MESSAGE(result == ORC_ERROR_NONE, "Int pop should succeed");
  TEST_ASSERT_TRUE_MESSAGE(int_value == 99, "Popped int value should be 99");
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_arr_len(int_arr) == 1,
                           "Int array should have 1 element left");
  orc_sdk_arr_free(int_arr);
  // Test with pointers
  const char  *strings[] = {"first", "second", "third"};
  const char **str_arr   = NULL;
  const char  *str_value = NULL;
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_arr_push(str_arr, strings[0]) == ORC_ERROR_NONE,
                           "String push should succeed");
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_arr_push(str_arr, strings[1]) == ORC_ERROR_NONE,
                           "String push should succeed");
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_arr_push(str_arr, strings[2]) == ORC_ERROR_NONE,
                           "String push should succeed");
  result = orc_sdk_arr_pop(str_arr, &str_value);
  TEST_ASSERT_TRUE_MESSAGE(result == ORC_ERROR_NONE, "String pop should succeed");
  TEST_ASSERT_TRUE_MESSAGE(str_value == strings[2], "Popped string should be 'third'");
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_arr_len(str_arr) == 2,
                           "String array should have 2 elements left");
  orc_sdk_arr_free(str_arr);
  // Test capacity behavior - capacity should not decrease on pop
  double *cap_arr = NULL;
  TEST_ASSERT_TRUE(ORC_ERROR_NONE == orc_sdk_arr_reserve(cap_arr, 10));
  size_t initial_capacity = _orc_sdk_arr_capacity(cap_arr);
  // Fill with some elements
  for (int i = 0; i < 5; i++) {
    orc_sdk_arr_push(cap_arr, (double)i);
  }
  // Pop all elements
  for (int i = 0; i < 5; i++) {
    orc_sdk_arr_pop(cap_arr, &value);
  }
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_arr_len(cap_arr) == 0, "Array should be empty");
  TEST_ASSERT_TRUE_MESSAGE(_orc_sdk_arr_capacity(cap_arr) == initial_capacity,
                           "Capacity should not decrease");
  orc_sdk_arr_free(cap_arr);
  // Test push after pop (ensure array is still usable)
  double *reuse_arr = NULL;
  orc_sdk_arr_push(reuse_arr, 1.0);
  orc_sdk_arr_push(reuse_arr, 2.0);
  orc_sdk_arr_pop(reuse_arr, &value);
  TEST_ASSERT_TRUE_MESSAGE(value == 2.0, "Popped value should be 2.0");
  orc_sdk_arr_push(reuse_arr, 3.0);
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_arr_len(reuse_arr) == 2,
                           "Array should have 2 elements");
  TEST_ASSERT_TRUE_MESSAGE(reuse_arr[0] == 1.0, "First element should be 1.0");
  TEST_ASSERT_TRUE_MESSAGE(reuse_arr[1] == 3.0, "Second element should be 3.0");
  orc_sdk_arr_free(reuse_arr);
}

void test_arr_fibonacci(void)
{
  uint32_t *fibo = NULL;
  TEST_ASSERT_TRUE(orc_sdk_arr_push(fibo, 1) == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(orc_sdk_arr_push(fibo, 1) == ORC_ERROR_NONE);
  for (size_t i = 0; i < 10; ++i) {
    size_t const len = orc_sdk_arr_len(fibo);
    TEST_ASSERT_TRUE(orc_sdk_arr_push(fibo, fibo[len - 2] + fibo[len - 1]) ==
                     ORC_ERROR_NONE);
  }
  TEST_ASSERT_TRUE(orc_sdk_arr_len(fibo) == 12);
  uint32_t const expected[12] = {1, 1, 2, 3, 5, 8, 13, 21, 34, 55, 89, 144};
  for (size_t i = 0; i < 12; ++i) {
    TEST_ASSERT_TRUE(fibo[i] == expected[i]);
  }
  orc_sdk_arr_free(fibo);
}

void test_arr_header_alignment(void)
{
  TEST_ASSERT_TRUE_MESSAGE(
    (sizeof(_OrcSdk_ArrHeader) % sizeof(_MaxAlignCompat)) == 0,
    "Array header must align with the platform's maximum alignment to be compatible "
    "with arbitrary types inside the container. This doesn't guarantee alignment with "
    "SIMD types. The containers are not meant to be used with SIMD types.");
}

// ========== Registry tests ==========

#define REGISTRY_TEST_N_THREADS 8
#define REGISTRY_TEST_N_IDS 64

// Stress-tests the mutex protecting the global registry. 8 threads run concurrently, each
// owning a disjoint slice of 64 IDs (thread N owns IDs [N*8, N*8+7]). Within its slice
// each thread inserts, verifies, then removes — so there are no conflicting writes
// between threads. The test catches deadlocks and data corruption caused by concurrent
// access to the shared hashmap, but does not test key-conflict resolution (which is
// correct-by-construction for a mutex-protected map).

static int _registry_thread_fn(void *arg)
{
  size_t const thread_idx = (size_t)(uintptr_t)arg;
  size_t const per_thread = REGISTRY_TEST_N_IDS / REGISTRY_TEST_N_THREADS;
  size_t const start      = thread_idx * per_thread;
  size_t const end        = start + per_thread;

  for (size_t i = start; i < end; ++i) {
    void *ptr = (void *)(uintptr_t)(i + 1);  // non-NULL sentinel
    orc_sdk_registry_insert((uint64_t)i, ptr);
  }
  for (size_t i = start; i < end; ++i) {
    if (!orc_sdk_registry_contains((uint64_t)i))
      return 1;
    void *got = orc_sdk_registry_get((uint64_t)i);
    if (got != (void *)(uintptr_t)(i + 1))
      return 2;
  }
  for (size_t i = start; i < end; ++i) {
    orc_sdk_registry_remove((uint64_t)i);
  }
  return 0;
}

void test_registry_multithreaded(void)
{
  orc_sdk_init(NULL, NULL);  // ensures mutex is initialized
  orc_sdk_registry_clear();  // start with a clean registry

  thrd_t threads[REGISTRY_TEST_N_THREADS];
  for (size_t i = 0; i < REGISTRY_TEST_N_THREADS; ++i) {
    thrd_create(&threads[i], _registry_thread_fn, (void *)(uintptr_t)i);
  }
  int all_ok = 1;
  for (size_t i = 0; i < REGISTRY_TEST_N_THREADS; ++i) {
    int result = 0;
    thrd_join(threads[i], &result);
    if (result != 0)
      all_ok = 0;
  }
  TEST_ASSERT_TRUE_MESSAGE(all_ok, "All registry threads should succeed");
  TEST_ASSERT_TRUE_MESSAGE(
    !orc_sdk_registry_contains(0),
    "Registry should be empty after all threads remove their entries");
}

void test_handle_alloc_uses_host_id(void)
{
  orc_sdk_init(NULL, NULL);
  orc_sdk_registry_clear();

  OrcHandle out = {0};
  out.handle    = 42;

  OrcError err = orc_sdk_handle_alloc(ORC_TYPE_F64, &out);
  TEST_ASSERT_TRUE_MESSAGE(err == ORC_ERROR_NONE, "Alloc should succeed");
  TEST_ASSERT_TRUE_MESSAGE(out.handle == 42, "handle field must remain 42");
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_registry_contains(42), "Registry must contain ID 42");
  TEST_ASSERT_TRUE_MESSAGE(out.items != NULL, "items pointer must be set");

  orc_sdk_handle_free(&out);
  TEST_ASSERT_TRUE_MESSAGE(out.handle == 42,
                           "Even after freeing, the handle should be the same.");
  TEST_ASSERT_TRUE_MESSAGE(!orc_sdk_registry_contains(42),
                           "Registry must not contain ID 42 after free");
}

void test_ensure_alloc_reuse(void)
{
  // ID in registry, same type → reuse: items pointer must not change.
  orc_sdk_init(NULL, NULL);
  orc_sdk_registry_clear();
  OrcHandle h = {0};
  h.handle    = 50;
  orc_sdk_handle_alloc(ORC_TYPE_F64, &h);
  void const *const original_items = h.items;

  OrcError err = orc_sdk_oh_ensure_alloc(ORC_TYPE_F64, &h);
  TEST_ASSERT_TRUE_MESSAGE(err == ORC_ERROR_NONE, "Same type: should succeed");
  TEST_ASSERT_TRUE_MESSAGE(h.items == original_items, "Same type: items must not change");
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_registry_contains(50),
                           "Same type: ID must still be in registry");

  orc_sdk_handle_free(&h);
}

void test_ensure_alloc_type_mismatch(void)
{
  // ID in registry, wrong type → free old, allocate new.
  orc_sdk_init(NULL, NULL);
  orc_sdk_registry_clear();
  OrcHandle h = {0};
  h.handle    = 51;
  orc_sdk_handle_alloc(ORC_TYPE_F64, &h);
  OrcError err = orc_sdk_oh_ensure_alloc(ORC_TYPE_U32, &h);
  TEST_ASSERT_TRUE_MESSAGE(err == ORC_ERROR_NONE, "Type mismatch: should succeed");
  TEST_ASSERT_TRUE_MESSAGE(h.type_id == ORC_TYPE_U32,
                           "Type mismatch: type_id must be updated");
  TEST_ASSERT_TRUE_MESSAGE(h.item_size == sizeof(uint32_t),
                           "Type mismatch: item_size must reflect new type");
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_registry_contains(51),
                           "Type mismatch: ID must still be in registry");
  orc_sdk_handle_free(&h);
}

void test_ensure_alloc_fresh(void)
{
  // ID not in registry, free_fn == NULL → plain allocate.
  orc_sdk_init(NULL, NULL);
  orc_sdk_registry_clear();
  OrcHandle h  = {0};
  h.handle     = 52;
  OrcError err = orc_sdk_oh_ensure_alloc(ORC_TYPE_F64, &h);
  TEST_ASSERT_TRUE_MESSAGE(err == ORC_ERROR_NONE, "Fresh alloc: should succeed");
  TEST_ASSERT_TRUE_MESSAGE(h.type_id == ORC_TYPE_F64, "Fresh alloc: type_id must be set");
  TEST_ASSERT_TRUE_MESSAGE(h.items != NULL, "Fresh alloc: items must be set");
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_registry_contains(52),
                           "Fresh alloc: ID must be in registry");
  orc_sdk_handle_free(&h);
}

static bool     _mock_free_called = false;
static OrcError _mock_free_fn(OrcHandle *const handle)
{
  _mock_free_called = true;
  handle->free_fn   = NULL;
  handle->items     = NULL;
  return ORC_ERROR_NONE;
}

void test_ensure_alloc_eviction(void)
{
  // ID not in registry, free_fn != NULL → evict foreign owner, then allocate.
  orc_sdk_init(NULL, NULL);
  orc_sdk_registry_clear();
  _mock_free_called = false;
  OrcHandle h       = {0};
  h.handle          = 53;
  h.free_fn         = _mock_free_fn;
  h.items           = (void *)1;  // non-null: simulates foreign plugin data
  OrcError err      = orc_sdk_oh_ensure_alloc(ORC_TYPE_F64, &h);
  TEST_ASSERT_TRUE_MESSAGE(err == ORC_ERROR_NONE, "Eviction: should succeed");
  TEST_ASSERT_TRUE_MESSAGE(_mock_free_called, "Eviction: foreign free_fn must be called");
  TEST_ASSERT_TRUE_MESSAGE(h.type_id == ORC_TYPE_F64, "Eviction: type_id must be set");
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_registry_contains(53),
                           "Eviction: ID must be in registry");

  orc_sdk_handle_free(&h);
}

// Hashmap tests

typedef struct
{
  size_t nslots;
  size_t nbuckets;
  size_t n_used;
  size_t n_removed;
} HMapStats;

static HMapStats _hmap_stats(void *ptr)
{
  _OrcSdk_HashTableHeader *h = _orc_sdk_hmap_header(ptr);
  if (h) {
    size_t const nbuckets = h->n_total / ORC_SDK_HMAP_BUCKET_SIZE;
    TEST_ASSERT_TRUE(nbuckets == _orc_sdk_arr_capacity(h->buckets));
    TEST_ASSERT_TRUE(nbuckets == orc_sdk_arr_len(h->buckets));
    return (HMapStats) {.nslots    = h->n_total,
                        .nbuckets  = nbuckets,
                        .n_used    = h->n_used,
                        .n_removed = h->n_removed};
  }
  return (HMapStats) {0};
}

void test_hmap_basic(void)
{
  typedef struct
  {
    int key;
    int value;
  } Entry;
  Entry *map = NULL;
  // Test empty map
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_len(map) == 0, "Empty map should have length 0");
  HMapStats stats = _hmap_stats(map);
  TEST_ASSERT_TRUE_MESSAGE(stats.n_used == 0, "Empty map should have 0 used slots");
  TEST_ASSERT_TRUE_MESSAGE(stats.nslots == 0, "Empty map should have 0 total slots");
  TEST_ASSERT_TRUE_MESSAGE(stats.nbuckets == 0, "Empty map should have 0 buckets");
  // Test single insertion
  int key1 = 42, val1 = 100;
  orc_sdk_hmap_put(map, key1, val1);
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_len(map) == 1,
                           "Map should have 1 element after insert");
  TEST_ASSERT_TRUE_MESSAGE(map[0].key == 42, "First entry key should be 42");
  TEST_ASSERT_TRUE_MESSAGE(map[0].value == 100, "First entry value should be 100");
  stats = _hmap_stats(map);
  TEST_ASSERT_TRUE_MESSAGE(stats.n_used == 1, "Should have 1 used slot after insert");
  TEST_ASSERT_TRUE_MESSAGE(stats.nslots == ORC_SDK_HMAP_BUCKET_SIZE,
                           "Initial size should be ORC_SDK_HMAP_BUCKET_SIZE slots");
  TEST_ASSERT_TRUE_MESSAGE(stats.nbuckets == 1, "Should have 1 bucket initially");
  TEST_ASSERT_TRUE_MESSAGE(stats.n_removed == 0, "Should have no removed entries");
  // Test update (same key)
  int val2 = 200;
  orc_sdk_hmap_put(map, key1, val2);
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_len(map) == 1,
                           "Map should still have 1 element after update");
  TEST_ASSERT_TRUE_MESSAGE(map[0].key == 42, "Key should remain 42");
  TEST_ASSERT_TRUE_MESSAGE(map[0].value == 200, "Value should be updated to 200");
  stats = _hmap_stats(map);
  TEST_ASSERT_TRUE_MESSAGE(stats.n_used == 1,
                           "Should still have 1 used slot after update");
  TEST_ASSERT_TRUE_MESSAGE(stats.nslots == ORC_SDK_HMAP_BUCKET_SIZE,
                           "Size should remain unchanged after update");
  TEST_ASSERT_TRUE_MESSAGE(stats.n_removed == 0,
                           "Update shouldn't create removed entries");
  // Test second insertion (different key)
  int key2 = 99, val3 = 300;
  orc_sdk_hmap_put(map, key2, val3);
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_len(map) == 2, "Map should have 2 elements");
  stats = _hmap_stats(map);
  TEST_ASSERT_TRUE_MESSAGE(stats.n_used == 2,
                           "Should have 2 used slots after second insert");
  TEST_ASSERT_TRUE_MESSAGE(stats.nslots == ORC_SDK_HMAP_BUCKET_SIZE,
                           "Size should still be initial size");
  TEST_ASSERT_TRUE_MESSAGE(stats.n_removed == 0, "Should still have no removed entries");
  // Verify both entries exist (order may vary due to hashing)
  bool found_42 = false, found_99 = false;
  for (size_t i = 0; i < orc_sdk_hmap_len(map); i++) {
    if (map[i].key == 42) {
      TEST_ASSERT_TRUE_MESSAGE(map[i].value == 200, "Key 42 should have value 200");
      found_42 = true;
    }
    else if (map[i].key == 99) {
      TEST_ASSERT_TRUE_MESSAGE(map[i].value == 300, "Key 99 should have value 300");
      found_99 = true;
    }
  }
  TEST_ASSERT_TRUE_MESSAGE(found_42, "Should find key 42 in map");
  TEST_ASSERT_TRUE_MESSAGE(found_99, "Should find key 99 in map");
  orc_sdk_hmap_free(map);
}

void test_hmap_growth(void)
{
  typedef struct
  {
    int key;
    int value;
  } Entry;
  Entry *map = NULL;
  // Insert enough elements to trigger growth (initial size is 8, growth at 75%)
  // So we need more than 6 elements to trigger growth
  for (int i = 0; i < 10; i++) {
    int key = i, value = i * 10;
    orc_sdk_hmap_put(map, key, value);
  }
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_len(map) == 10,
                           "All elements should be inserted");
  // Verify all elements are present
  bool found[10] = {false};
  for (size_t i = 0; i < orc_sdk_hmap_len(map); i++) {
    int key            = map[i].key;
    int expected_value = key * 10;
    TEST_ASSERT_TRUE_MESSAGE(key >= 0 && key < 10, "Key should be in valid range");
    TEST_ASSERT_TRUE_MESSAGE(map[i].value == expected_value,
                             "Value should match expected");
    found[key] = true;
  }
  for (int i = 0; i < 10; i++) {
    TEST_ASSERT_TRUE_MESSAGE(found[i], "All keys should be found after growth");
  }
  orc_sdk_hmap_free(map);
}

void test_hmap_edge_cases(void)
{
  typedef struct
  {
    int key;
    int value;
  } Entry;
  Entry *map = NULL;
  // Test with key 0 (potential edge case)
  int key0 = 0, val0 = 999;
  orc_sdk_hmap_put(map, key0, val0);
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_len(map) == 1, "Should handle key 0");
  TEST_ASSERT_TRUE_MESSAGE(map[0].key == 0, "Key 0 should be stored correctly");
  TEST_ASSERT_TRUE_MESSAGE(map[0].value == 999, "Value for key 0 should be correct");
  // Test with negative keys
  int key_neg = -1, val_neg = -999;
  orc_sdk_hmap_put(map, key_neg, val_neg);
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_len(map) == 2, "Should handle negative keys");
  // Test with large keys
  int key_large = 1000000, val_large = 123;
  orc_sdk_hmap_put(map, key_large, val_large);
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_len(map) == 3, "Should handle large keys");
  // Test multiple updates to same key
  int key_repeat = 42;
  for (int i = 0; i < 5; i++) {
    orc_sdk_hmap_put(map, key_repeat, i);
    TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_len(map) == 4,
                             "Multiple updates shouldn't increase size");
  }
  // Find key 42 and verify final value
  bool found_42 = false;
  for (size_t i = 0; i < orc_sdk_hmap_len(map); i++) {
    if (map[i].key == 42) {
      TEST_ASSERT_TRUE_MESSAGE(map[i].value == 4, "Final update value should be 4");
      found_42 = true;
      break;
    }
  }
  TEST_ASSERT_TRUE_MESSAGE(found_42, "Should find key 42 after multiple updates");
  orc_sdk_hmap_free(map);
}

void test_hmap_null_operations(void)
{
  typedef struct
  {
    int key;
    int value;
  } Entry;
  Entry *null_map = NULL;
  // Test length of NULL map
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_len(null_map) == 0,
                           "NULL map should have length 0");
  // Test that we can insert into NULL map (should initialize)
  int key = 1, value = 100;
  orc_sdk_hmap_put(null_map, key, value);
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_len(null_map) == 1,
                           "Should initialize NULL map on first insert");
  TEST_ASSERT_TRUE_MESSAGE(null_map[0].key == 1, "First key should be correct");
  TEST_ASSERT_TRUE_MESSAGE(null_map[0].value == 100, "First value should be correct");
  orc_sdk_hmap_free(null_map);
}

void test_hmap_stress_test(void)
{
  typedef struct
  {
    int key;
    int value;
  } Entry;
  Entry *map = NULL;
  // Test many insertions to stress growth/rehashing
  // Insert many elements
  for (int i = 0; i < 1000; i++) {
    int key = i, value = i * 2;
    orc_sdk_hmap_put(map, key, value);
  }
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_len(map) == 1000,
                           "Should handle many insertions");
  HMapStats stats = _hmap_stats(map);
  TEST_ASSERT_TRUE_MESSAGE(stats.n_used == 1000, "Stats should match actual count");
  TEST_ASSERT_TRUE_MESSAGE(stats.n_removed == 0,
                           "Should have no removed entries after insertions");
  // Size should be at least large enough, and a multiple of ORC_SDK_HMAP_BUCKET_SIZE
  TEST_ASSERT_TRUE_MESSAGE(stats.nslots >= 1000,
                           "Should have grown to accommodate all elements");
  TEST_ASSERT_TRUE_MESSAGE(stats.nslots % ORC_SDK_HMAP_BUCKET_SIZE == 0,
                           "Slot count should be multiple of bucket size");
  TEST_ASSERT_TRUE_MESSAGE(stats.nbuckets == stats.nslots / ORC_SDK_HMAP_BUCKET_SIZE,
                           "Bucket count should be consistent");
  // Verify all elements are still there after multiple growths
  bool found[1000] = {false};
  for (size_t i = 0; i < orc_sdk_hmap_len(map); i++) {
    int key            = map[i].key;
    int expected_value = key * 2;
    TEST_ASSERT_TRUE_MESSAGE(key >= 0 && key < 1000, "Key should be in valid range");
    TEST_ASSERT_TRUE_MESSAGE(map[i].value == expected_value,
                             "Value should be correct after growth");
    found[key] = true;
  }
  // Check that all keys were found
  for (int i = 0; i < 1000; i++) {
    TEST_ASSERT_TRUE_MESSAGE(found[i], "All keys should survive multiple growths");
  }
  // Test many updates
  for (int i = 0; i < 1000; i++) {
    int key = i, new_value = i * 3;
    orc_sdk_hmap_put(map, key, new_value);
  }
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_len(map) == 1000,
                           "Length should remain same after updates");
  stats = _hmap_stats(map);
  TEST_ASSERT_TRUE_MESSAGE(stats.n_used == 1000,
                           "Used count should remain same after updates");
  TEST_ASSERT_TRUE_MESSAGE(stats.n_removed == 0,
                           "Updates shouldn't create removed entries");
  // Verify updates worked
  for (size_t i = 0; i < orc_sdk_hmap_len(map); i++) {
    int key            = map[i].key;
    int expected_value = key * 3;
    TEST_ASSERT_TRUE_MESSAGE(map[i].value == expected_value,
                             "Updated values should be correct");
  }
  orc_sdk_hmap_free(map);
}

void test_hmap_hash_collision_simulation(void)
{
  typedef struct
  {
    int key;
    int value;
  } Entry;
  Entry *map = NULL;
  // Test keys that are likely to cause hash collisions
  // Use multiples of large numbers to increase collision probability
  int collision_keys[] = {
    0,
    8,
    16,
    24,
    32,
    40,
    48,
    56,
    64,
    72,  // Multiples of 8
    1024,
    2048,
    4096,
    8192,
    16384,  // Powers of 2
    -1,
    -8,
    -16,
    -24  // Negative multiples
  };
  int num_keys = sizeof(collision_keys) / sizeof(collision_keys[0]);
  // Insert all collision-prone keys
  for (int i = 0; i < num_keys; i++) {
    int key = collision_keys[i], value = i + 100;
    orc_sdk_hmap_put(map, key, value);
  }
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_len(map) == (size_t)num_keys,
                           "Should handle potential hash collisions");
  // Verify all keys are present and correct
  for (int i = 0; i < num_keys; i++) {
    int  target_key     = collision_keys[i];
    int  expected_value = i + 100;
    bool found          = false;
    for (size_t j = 0; j < orc_sdk_hmap_len(map); j++) {
      if (map[j].key == target_key) {
        TEST_ASSERT_TRUE_MESSAGE(map[j].value == expected_value,
                                 "Collision key should have correct value");
        found = true;
        break;
      }
    }
    TEST_ASSERT_TRUE_MESSAGE(found, "All collision-prone keys should be found");
  }
  orc_sdk_hmap_free(map);
}

void test_hmap_boundary_conditions(void)
{
  typedef struct
  {
    int key;
    int value;
  } Entry;
  Entry *map = NULL;
  // Test exactly at growth boundaries
  // Initial size is ORC_SDK_HMAP_BUCKET_SIZE, grows at 75% = 6 elements (assuming
  // ORC_SDK_HMAP_BUCKET_SIZE=8)
  // Insert exactly to growth threshold
  for (int i = 0; i < 6; i++) {
    int key = i, value = i;
    orc_sdk_hmap_put(map, key, value);
  }
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_len(map) == 6,
                           "Should handle exactly 6 elements");
  HMapStats stats = _hmap_stats(map);
  TEST_ASSERT_TRUE_MESSAGE(stats.n_used == 6, "Should have 6 used slots at threshold");
  TEST_ASSERT_TRUE_MESSAGE(stats.nslots == ORC_SDK_HMAP_BUCKET_SIZE,
                           "Should still be initial size before growth");
  TEST_ASSERT_TRUE_MESSAGE(stats.nbuckets == 1,
                           "Should still have 1 bucket before growth");
  TEST_ASSERT_TRUE_MESSAGE(stats.n_removed == 0, "Should have no removed entries");
  // One more should trigger growth
  int key7 = 100, val7 = 200;
  orc_sdk_hmap_put(map, key7, val7);
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_len(map) == 7, "Should handle growth trigger");
  stats = _hmap_stats(map);
  TEST_ASSERT_TRUE_MESSAGE(stats.n_used == 7, "Should have 7 used slots after growth");
  TEST_ASSERT_TRUE_MESSAGE(stats.nslots == ORC_SDK_HMAP_BUCKET_SIZE * 2,
                           "Should double in size after growth");
  TEST_ASSERT_TRUE_MESSAGE(stats.nbuckets == 2, "Should have 2 buckets after growth");
  TEST_ASSERT_TRUE_MESSAGE(stats.n_removed == 0,
                           "Growth should reset removed count to 0");
  // Verify all elements survive growth
  bool found[7]  = {false};
  bool found_100 = false;
  for (size_t i = 0; i < orc_sdk_hmap_len(map); i++) {
    if (map[i].key == 100) {
      TEST_ASSERT_TRUE_MESSAGE(map[i].value == 200,
                               "Growth trigger element should be correct");
      found_100 = true;
    }
    else if (map[i].key >= 0 && map[i].key < 6) {
      found[map[i].key] = true;
      TEST_ASSERT_TRUE_MESSAGE(map[i].value == map[i].key,
                               "Original elements should survive growth");
    }
  }
  TEST_ASSERT_TRUE_MESSAGE(found_100, "Growth trigger element should be found");
  for (int i = 0; i < 6; i++) {
    TEST_ASSERT_TRUE_MESSAGE(found[i], "All original elements should survive growth");
  }
  orc_sdk_hmap_free(map);
}

void test_hmap_extreme_values(void)
{
  typedef struct
  {
    int       key;
    long long value;  // Use larger value type
  } Entry;
  Entry *map = NULL;
  // Test extreme integer values
  int extreme_keys[] = {
    INT_MAX,
    INT_MIN,
    0,
    -1,
    1,
    INT_MAX - 1,
    INT_MIN + 1,
    32767,
    -32768,  // 16-bit boundaries
    65535,
    -65536  // Around 16-bit unsigned boundary
  };
  int num_keys = sizeof(extreme_keys) / sizeof(extreme_keys[0]);
  for (int i = 0; i < num_keys; i++) {
    int       key   = extreme_keys[i];
    long long value = (long long)key * 1000000LL;  // Large values
    orc_sdk_hmap_put(map, key, value);
  }
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_len(map) == (size_t)num_keys,
                           "Should handle extreme key values");
  // Verify extreme values
  for (int i = 0; i < num_keys; i++) {
    int       target_key     = extreme_keys[i];
    long long expected_value = (long long)target_key * 1000000LL;
    bool      found          = false;
    for (size_t j = 0; j < orc_sdk_hmap_len(map); j++) {
      if (map[j].key == target_key) {
        TEST_ASSERT_TRUE_MESSAGE(map[j].value == expected_value,
                                 "Extreme value should be correct");
        found = true;
        break;
      }
    }
    TEST_ASSERT_TRUE_MESSAGE(found, "Extreme key should be found");
  }
  orc_sdk_hmap_free(map);
}

void test_hmap_repeated_growth(void)
{
  typedef struct
  {
    int key;
    int value;
  } Entry;
  Entry *map = NULL;
  // Force multiple growth cycles by inserting in phases
  int phase_sizes[]  = {5, 10, 20, 50, 100};
  int num_phases     = sizeof(phase_sizes) / sizeof(phase_sizes[0]);
  int total_inserted = 0;
  for (int phase = 0; phase < num_phases; phase++) {
    int phase_size = phase_sizes[phase];
    // Insert elements for this phase
    for (int i = 0; i < phase_size; i++) {
      int key   = total_inserted + i;
      int value = key * 10 + phase;  // Make value depend on both key and phase
      orc_sdk_hmap_put(map, key, value);
    }
    total_inserted += phase_size;
    TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_len(map) == (size_t)total_inserted,
                             "Length should match total insertions");
    // Verify all previous elements are still correct after each growth
    for (int check_key = 0; check_key < total_inserted; check_key++) {
      bool found = false;
      for (size_t j = 0; j < orc_sdk_hmap_len(map); j++) {
        if (map[j].key == check_key) {
          // Calculate expected value based on which phase this key was from
          int key_phase     = 0;
          int running_total = 0;
          for (int p = 0; p < num_phases; p++) {
            if (check_key < running_total + phase_sizes[p]) {
              key_phase = p;
              break;
            }
            running_total += phase_sizes[p];
          }
          int expected_value = check_key * 10 + key_phase;
          TEST_ASSERT_TRUE_MESSAGE(map[j].value == expected_value,
                                   "Value should survive multiple growths");
          found = true;
          break;
        }
      }
      TEST_ASSERT_TRUE_MESSAGE(found, "All keys should survive repeated growths");
    }
  }
  orc_sdk_hmap_free(map);
}

void test_hmap_get_basic(void)
{
  typedef struct
  {
    int key;
    int value;
  } Entry;
  Entry *map = NULL;
  // Test get from empty map
  int    key    = 42;
  Entry *result = (Entry *)orc_sdk_hmap_get(map, key);
  TEST_ASSERT_TRUE_MESSAGE(result == NULL, "Get from empty map should return NULL");
  TEST_ASSERT_TRUE_MESSAGE(!orc_sdk_hmap_contains(map, key),
                           "Empty map should not contain any key");
  // Insert some entries
  int key1 = 10, val1 = 100;
  int key2 = 20, val2 = 200;
  int key3 = 30, val3 = 300;
  orc_sdk_hmap_put(map, key1, val1);
  orc_sdk_hmap_put(map, key2, val2);
  orc_sdk_hmap_put(map, key3, val3);
  // Test orc_sdk_hmap_contains
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_contains(map, key1), "Should contain key1");
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_contains(map, key2), "Should contain key2");
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_contains(map, key3), "Should contain key3");
  int missing_key1 = 999;
  TEST_ASSERT_TRUE_MESSAGE(!orc_sdk_hmap_contains(map, missing_key1),
                           "Should not contain missing key");
  // Test successful gets
  result = (Entry *)orc_sdk_hmap_get(map, key1);
  TEST_ASSERT_TRUE_MESSAGE(result != NULL, "Should find existing key 10");
  TEST_ASSERT_TRUE_MESSAGE(result->key == 10, "Retrieved entry should have correct key");
  TEST_ASSERT_TRUE_MESSAGE(result->value == 100,
                           "Retrieved entry should have correct value");
  result = (Entry *)orc_sdk_hmap_get(map, key2);
  TEST_ASSERT_TRUE_MESSAGE(result != NULL, "Should find existing key 20");
  TEST_ASSERT_TRUE_MESSAGE(result->key == 20, "Retrieved entry should have correct key");
  TEST_ASSERT_TRUE_MESSAGE(result->value == 200,
                           "Retrieved entry should have correct value");
  result = (Entry *)orc_sdk_hmap_get(map, key3);
  TEST_ASSERT_TRUE_MESSAGE(result != NULL, "Should find existing key 30");
  TEST_ASSERT_TRUE_MESSAGE(result->key == 30, "Retrieved entry should have correct key");
  TEST_ASSERT_TRUE_MESSAGE(result->value == 300,
                           "Retrieved entry should have correct value");
  // Test missing key
  int missing_key = 999;
  result          = (Entry *)orc_sdk_hmap_get(map, missing_key);
  TEST_ASSERT_TRUE_MESSAGE(result == NULL, "Should return NULL for missing key");
  TEST_ASSERT_TRUE_MESSAGE(!orc_sdk_hmap_contains(map, missing_key),
                           "Should not contain missing key");
  orc_sdk_hmap_free(map);
}

void test_hmap_get_after_updates(void)
{
  typedef struct
  {
    int key;
    int value;
  } Entry;
  Entry *map = NULL;
  // Insert initial value
  int key = 42, initial_val = 100;
  orc_sdk_hmap_put(map, key, initial_val);
  // Verify initial state
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_contains(map, key),
                           "Should contain key after insert");
  Entry *result = (Entry *)orc_sdk_hmap_get(map, key);
  TEST_ASSERT_TRUE_MESSAGE(result != NULL, "Should find key after initial insert");
  TEST_ASSERT_TRUE_MESSAGE(result->value == 100, "Should have initial value");
  // Update the value
  int updated_val = 999;
  orc_sdk_hmap_put(map, key, updated_val);
  // Verify state after update
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_contains(map, key),
                           "Should still contain key after update");
  result = (Entry *)orc_sdk_hmap_get(map, key);
  TEST_ASSERT_TRUE_MESSAGE(result != NULL, "Should still find key after update");
  TEST_ASSERT_TRUE_MESSAGE(result->value == 999, "Should have updated value");
  TEST_ASSERT_TRUE_MESSAGE(result->key == 42, "Key should remain unchanged");
  // Verify map still has only one entry
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_len(map) == 1,
                           "Map should still have only one entry after update");
  orc_sdk_hmap_free(map);
}

void test_hmap_get_with_collisions(void)
{
  typedef struct
  {
    int key;
    int value;
  } Entry;
  Entry *map = NULL;
  // Use keys that are likely to cause collisions
  int collision_keys[] = {0, 8, 16, 24, 32};  // Multiples of 8
  int values[]         = {100, 200, 300, 400, 500};
  int num_keys         = sizeof(collision_keys) / sizeof(collision_keys[0]);
  // Insert collision-prone keys
  for (int i = 0; i < num_keys; i++) {
    orc_sdk_hmap_put(map, collision_keys[i], values[i]);
  }
  // Verify all keys are contained
  for (int i = 0; i < num_keys; i++) {
    TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_contains(map, collision_keys[i]),
                             "Should contain collision-prone key");
  }
  // Verify all keys can be retrieved correctly
  for (int i = 0; i < num_keys; i++) {
    Entry *result = (Entry *)orc_sdk_hmap_get(map, collision_keys[i]);
    TEST_ASSERT_TRUE_MESSAGE(result != NULL, "Should find collision-prone key");
    TEST_ASSERT_TRUE_MESSAGE(result->key == collision_keys[i],
                             "Retrieved key should match");
    TEST_ASSERT_TRUE_MESSAGE(result->value == values[i], "Retrieved value should match");
  }
  // Test missing keys that might hash to same buckets
  int missing_keys[] = {40, 48, 56};
  for (int i = 0; i < 3; i++) {
    TEST_ASSERT_TRUE_MESSAGE(!orc_sdk_hmap_contains(map, missing_keys[i]),
                             "Should not contain missing collision-candidate key");
    Entry *result = (Entry *)orc_sdk_hmap_get(map, missing_keys[i]);
    TEST_ASSERT_TRUE_MESSAGE(result == NULL,
                             "Should not find missing collision-candidate key");
  }
  orc_sdk_hmap_free(map);
}

void test_hmap_get_after_growth(void)
{
  typedef struct
  {
    int key;
    int value;
  } Entry;
  Entry *map = NULL;
  // Insert elements before growth
  int pre_growth_keys[]   = {1, 2, 3, 4, 5};
  int pre_growth_values[] = {10, 20, 30, 40, 50};
  for (int i = 0; i < 5; i++) {
    orc_sdk_hmap_put(map, pre_growth_keys[i], pre_growth_values[i]);
  }
  // Verify pre-growth containment and retrieval
  for (int i = 0; i < 5; i++) {
    TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_contains(map, pre_growth_keys[i]),
                             "Should contain pre-growth key");
    Entry *result = (Entry *)orc_sdk_hmap_get(map, pre_growth_keys[i]);
    TEST_ASSERT_TRUE_MESSAGE(result != NULL, "Should find pre-growth key");
    TEST_ASSERT_TRUE_MESSAGE(result->value == pre_growth_values[i],
                             "Pre-growth value should be correct");
  }
  // Trigger growth by adding more elements (assuming 8 initial size, 75% threshold)
  int post_growth_keys[]   = {6, 7, 8, 9, 10};
  int post_growth_values[] = {60, 70, 80, 90, 100};
  for (int i = 0; i < 5; i++) {
    orc_sdk_hmap_put(map, post_growth_keys[i], post_growth_values[i]);
  }
  // Verify all pre-growth entries still accessible after growth
  for (int i = 0; i < 5; i++) {
    TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_contains(map, pre_growth_keys[i]),
                             "Should contain pre-growth key after growth");
    Entry *result = (Entry *)orc_sdk_hmap_get(map, pre_growth_keys[i]);
    TEST_ASSERT_TRUE_MESSAGE(result != NULL, "Should find pre-growth key after growth");
    TEST_ASSERT_TRUE_MESSAGE(result->value == pre_growth_values[i],
                             "Pre-growth value should survive growth");
  }
  // Verify post-growth entries
  for (int i = 0; i < 5; i++) {
    TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_contains(map, post_growth_keys[i]),
                             "Should contain post-growth key");
    Entry *result = (Entry *)orc_sdk_hmap_get(map, post_growth_keys[i]);
    TEST_ASSERT_TRUE_MESSAGE(result != NULL, "Should find post-growth key");
    TEST_ASSERT_TRUE_MESSAGE(result->value == post_growth_values[i],
                             "Post-growth value should be correct");
  }
  orc_sdk_hmap_free(map);
}

void test_hmap_get_edge_cases(void)
{
  typedef struct
  {
    int       key;
    long long value;  // Different value type
  } Entry;
  Entry *map = NULL;
  // Test with extreme key values
  int       extreme_keys[]   = {INT_MAX, INT_MIN, 0, -1, 1};
  long long extreme_values[] = {1000000LL, -1000000LL, 0LL, -1LL, 1LL};
  for (int i = 0; i < 5; i++) {
    orc_sdk_hmap_put(map, extreme_keys[i], extreme_values[i]);
  }
  // Verify extreme values with both contains and get
  for (int i = 0; i < 5; i++) {
    TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_contains(map, extreme_keys[i]),
                             "Should contain extreme key");
    Entry *result = (Entry *)orc_sdk_hmap_get(map, extreme_keys[i]);
    TEST_ASSERT_TRUE_MESSAGE(result != NULL, "Should find extreme key");
    TEST_ASSERT_TRUE_MESSAGE(result->key == extreme_keys[i], "Extreme key should match");
    TEST_ASSERT_TRUE_MESSAGE(result->value == extreme_values[i],
                             "Extreme value should match");
  }
  // Test key 0 specifically (potential edge case)
  int zero_key = 0;
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_contains(map, zero_key), "Should contain key 0");
  Entry *result = (Entry *)orc_sdk_hmap_get(map, zero_key);
  TEST_ASSERT_TRUE_MESSAGE(result != NULL, "Should find key 0");
  TEST_ASSERT_TRUE_MESSAGE(result->key == 0, "Key 0 should be retrievable");
  TEST_ASSERT_TRUE_MESSAGE(result->value == 0LL, "Value for key 0 should be correct");
  orc_sdk_hmap_free(map);
}

void test_hmap_get_null_safety(void)
{
  typedef struct
  {
    int key;
    int value;
  } Entry;
  // Test operations on NULL map
  Entry *null_map      = NULL;
  int    test_key_null = 42;
  TEST_ASSERT_TRUE_MESSAGE(!orc_sdk_hmap_contains(null_map, test_key_null),
                           "NULL map should not contain any key");
  Entry *result = (Entry *)orc_sdk_hmap_get(null_map, test_key_null);
  TEST_ASSERT_TRUE_MESSAGE(result == NULL, "Get from NULL map should return NULL");
  // Test get from map that works normally
  Entry *map = NULL;
  int    key = 123, value = 456;
  orc_sdk_hmap_put(map, key, value);
  // Verify it works before free
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_contains(map, key),
                           "Should contain key before free");
  result = (Entry *)orc_sdk_hmap_get(map, key);
  TEST_ASSERT_TRUE_MESSAGE(result != NULL, "Should find key before free");
  orc_sdk_hmap_free(map);
  // Note: Don't test after free as that would be undefined behavior
}

void test_hmap_fibo_indices(void)
{
  typedef struct
  {
    uint64_t key;
    uint64_t value;
  } Entry;
  uint64_t *fibo = NULL;
  {  // Populate array.
    TEST_ASSERT_TRUE_MESSAGE(orc_sdk_arr_push(fibo, 1) == ORC_ERROR_NONE,
                             "Failed to push to array");
    TEST_ASSERT_TRUE_MESSAGE(orc_sdk_arr_push(fibo, 1) == ORC_ERROR_NONE,
                             "Failed to push to array");
    for (size_t i = 0; i < 50; ++i) {
      size_t const len = orc_sdk_arr_len(fibo);
      TEST_ASSERT_TRUE_MESSAGE(
        orc_sdk_arr_push(fibo, fibo[len - 2] + fibo[len - 1]) == ORC_ERROR_NONE,
        "Array length is not correct.");
    }
    TEST_ASSERT_TRUE_MESSAGE(orc_sdk_arr_len(fibo) == 52,
                             "Not enough fibonacci numbers.");
    orc_sdk_arr_remove(fibo, 0);  // Remove the duplicated 1.
    TEST_ASSERT_TRUE_MESSAGE(orc_sdk_arr_len(fibo) == 51,
                             "One less after removing the duplicate");
  }
  Entry *idxmap = NULL;
  {  // Populate the map.
    size_t const len = orc_sdk_arr_len(fibo);
    uint64_t    *fn  = fibo;
    for (size_t i = 0; i < len; ++i, ++fn) {
      orc_sdk_hmap_put(idxmap, *fn, i);
      TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_len(idxmap) == (i + 1),
                               "Hasmap size is not growing.");
    }
  }
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_len(idxmap) == orc_sdk_arr_len(fibo),
                           "Hashmap o fibonacci numbers is not the right size.");
  {  // Check the mapping.
    size_t const len = orc_sdk_arr_len(fibo);
    uint64_t    *fn  = fibo;
    for (size_t i = 0; i < len; ++i, ++fn) {
      Entry *match = (Entry *)orc_sdk_hmap_get(idxmap, *fn);
      TEST_ASSERT_TRUE_MESSAGE(match != NULL, "Match must be found.");
      TEST_ASSERT_TRUE_MESSAGE(match->key == *fn && match->value == i,
                               "Match doesn't actually match");
    }
  }
  orc_sdk_hmap_free(idxmap);
  orc_sdk_arr_free(fibo);
}

void test_hmap_remove_basic(void)
{
  typedef struct
  {
    int key;
    int value;
  } Entry;
  Entry *map = NULL;
  // Insert some elements
  int key1 = 10, val1 = 100;
  int key2 = 20, val2 = 200;
  int key3 = 30, val3 = 300;
  orc_sdk_hmap_put(map, key1, val1);
  orc_sdk_hmap_put(map, key2, val2);
  orc_sdk_hmap_put(map, key3, val3);
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_len(map) == 3,
                           "Should have 3 elements before removal");
  HMapStats stats = _hmap_stats(map);
  TEST_ASSERT_TRUE_MESSAGE(stats.n_used == 3, "Should have 3 used slots");
  TEST_ASSERT_TRUE_MESSAGE(stats.n_removed == 0, "Should have 0 removed slots initially");
  // Test successful removal
  orc_sdk_hmap_remove(map, key2);
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_len(map) == 2,
                           "Should have 2 elements after removal");
  TEST_ASSERT_TRUE_MESSAGE(!orc_sdk_hmap_contains(map, key2),
                           "Should not contain removed key");
  Entry *result = (Entry *)orc_sdk_hmap_get(map, key2);
  TEST_ASSERT_TRUE_MESSAGE(result == NULL, "Get should return NULL for removed key");
  stats = _hmap_stats(map);
  TEST_ASSERT_TRUE_MESSAGE(stats.n_used == 2, "Should have 2 used slots after removal");
  TEST_ASSERT_TRUE_MESSAGE(stats.n_removed <= 1,
                           "Should have at most 1 removed slot after removal");
  // Verify other elements still exist
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_contains(map, key1), "Should still contain key1");
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_contains(map, key3), "Should still contain key3");
  result = (Entry *)orc_sdk_hmap_get(map, key1);
  TEST_ASSERT_TRUE_MESSAGE(result != NULL, "Should find key1");
  TEST_ASSERT_TRUE_MESSAGE(result->value == val1, "Key1 should have correct value");
  result = (Entry *)orc_sdk_hmap_get(map, key3);
  TEST_ASSERT_TRUE_MESSAGE(result != NULL, "Should find key3");
  TEST_ASSERT_TRUE_MESSAGE(result->value == val3, "Key3 should have correct value");
  // Remove remaining elements
  orc_sdk_hmap_remove(map, key1);
  orc_sdk_hmap_remove(map, key3);
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_len(map) == 0,
                           "Should be empty after removing all");
  stats = _hmap_stats(map);
  TEST_ASSERT_TRUE_MESSAGE(stats.n_used == 0, "Should have 0 used slots when empty");
  TEST_ASSERT_TRUE_MESSAGE(stats.n_removed <= 3,
                           "Should have at most 3 removed slots (may compact)");
  orc_sdk_hmap_free(map);
}

void test_hmap_remove_nonexistent(void)
{
  typedef struct
  {
    int key;
    int value;
  } Entry;
  Entry *map = NULL;
  // Test removing from empty map
  int missing_key = 999;
  orc_sdk_hmap_remove(map, missing_key);  // Should do nothing
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_len(map) == 0, "Empty map should remain empty");
  // Insert some elements
  int key1 = 10, val1 = 100;
  int key2 = 20, val2 = 200;
  orc_sdk_hmap_put(map, key1, val1);
  orc_sdk_hmap_put(map, key2, val2);
  HMapStats stats           = _hmap_stats(map);
  size_t    initial_used    = stats.n_used;
  size_t    initial_removed = stats.n_removed;
  size_t    initial_len     = orc_sdk_hmap_len(map);
  // Try to remove non-existent keys
  int nonexistent_keys[] = {5, 15, 25, 999, -1, 0};
  for (int i = 0; i < 6; i++) {
    orc_sdk_hmap_remove(map, nonexistent_keys[i]);
    // Verify map state unchanged
    TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_len(map) == initial_len,
                             "Length should not change");
    stats = _hmap_stats(map);
    TEST_ASSERT_TRUE_MESSAGE(stats.n_used == initial_used,
                             "Used count should not change");
    TEST_ASSERT_TRUE_MESSAGE(stats.n_removed == initial_removed,
                             "Removed count should not change");
    // Verify existing keys still present
    TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_contains(map, key1),
                             "Existing key1 should still be present");
    TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_contains(map, key2),
                             "Existing key2 should still be present");
  }
  // Remove an existing key, then try to remove it again
  orc_sdk_hmap_remove(map, key1);
  TEST_ASSERT_TRUE_MESSAGE(!orc_sdk_hmap_contains(map, key1), "Key1 should be removed");
  stats                        = _hmap_stats(map);
  size_t used_after_removal    = stats.n_used;
  size_t removed_after_removal = stats.n_removed;
  // Try to remove the already removed key
  orc_sdk_hmap_remove(map, key1);
  stats = _hmap_stats(map);
  TEST_ASSERT_TRUE_MESSAGE(
    stats.n_used == used_after_removal,
    "Used count should not change when removing already removed key");
  TEST_ASSERT_TRUE_MESSAGE(
    stats.n_removed == removed_after_removal,
    "Removed count should not change when removing already removed key");
  TEST_ASSERT_TRUE_MESSAGE(!orc_sdk_hmap_contains(map, key1),
                           "Key1 should still not be present");
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_contains(map, key2),
                           "Key2 should still be present");
  orc_sdk_hmap_free(map);
}

void test_hmap_remove_returns_bool(void)
{
  typedef struct
  {
    int key;
    int value;
  } Entry;
  Entry *map = NULL;
  int    key = 42, val = 1;
  // Remove from empty map — must return false.
  TEST_ASSERT_TRUE_MESSAGE(!orc_sdk_hmap_remove(map, key),
                           "Remove from empty map should return false");
  orc_sdk_hmap_put(map, key, val);
  // Remove existing key — must return true.
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_remove(map, key),
                           "Remove of present key should return true");
  // Remove same key again — must return false.
  TEST_ASSERT_TRUE_MESSAGE(!orc_sdk_hmap_remove(map, key),
                           "Remove of already-removed key should return false");
  orc_sdk_hmap_free(map);
}

void test_hmap_remove_with_collisions(void)
{
  typedef struct
  {
    int key;
    int value;
  } Entry;
  Entry *map = NULL;
  // Use keys that are likely to cause collisions
  int collision_keys[] = {0, 8, 16, 24, 32, 40};  // Multiples of 8
  int values[]         = {100, 200, 300, 400, 500, 600};
  int num_keys         = sizeof(collision_keys) / sizeof(collision_keys[0]);
  // Insert collision-prone keys
  for (int i = 0; i < num_keys; i++) {
    orc_sdk_hmap_put(map, collision_keys[i], values[i]);
  }
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_len(map) == (size_t)num_keys,
                           "Should have all keys initially");
  // Remove middle element (should be in a collision chain)
  int removed_key   = collision_keys[2];  // key = 16
  int removed_value = values[2];          // value = 300
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_contains(map, removed_key),
                           "Key should exist before removal");
  Entry *result = (Entry *)orc_sdk_hmap_get(map, removed_key);
  TEST_ASSERT_TRUE_MESSAGE(result != NULL && result->value == removed_value,
                           "Should find correct value before removal");
  orc_sdk_hmap_remove(map, removed_key);
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_len(map) == (size_t)num_keys - 1,
                           "Length should decrease by 1");
  TEST_ASSERT_TRUE_MESSAGE(!orc_sdk_hmap_contains(map, removed_key),
                           "Removed key should not be found");
  result = (Entry *)orc_sdk_hmap_get(map, removed_key);
  TEST_ASSERT_TRUE_MESSAGE(result == NULL, "Get should return NULL for removed key");
  // Verify all other collision-prone keys are still accessible
  for (int i = 0; i < num_keys; i++) {
    if (collision_keys[i] == removed_key)
      continue;
    TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_contains(map, collision_keys[i]),
                             "Other collision keys should still be present");
    result = (Entry *)orc_sdk_hmap_get(map, collision_keys[i]);
    TEST_ASSERT_TRUE_MESSAGE(result != NULL, "Should find other collision keys");
    TEST_ASSERT_TRUE_MESSAGE(result->key == collision_keys[i], "Key should match");
    TEST_ASSERT_TRUE_MESSAGE(result->value == values[i], "Value should match");
  }
  // Remove first element in potential chain
  orc_sdk_hmap_remove(map, collision_keys[0]);
  TEST_ASSERT_TRUE_MESSAGE(!orc_sdk_hmap_contains(map, collision_keys[0]),
                           "First key should be removed");
  // Remove last element in potential chain
  orc_sdk_hmap_remove(map, collision_keys[num_keys - 1]);
  TEST_ASSERT_TRUE_MESSAGE(!orc_sdk_hmap_contains(map, collision_keys[num_keys - 1]),
                           "Last key should be removed");
  // Verify remaining elements are still accessible
  for (int i = 1; i < num_keys - 1; i++) {
    if (collision_keys[i] == removed_key)
      continue;
    TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_contains(map, collision_keys[i]),
                             "Remaining collision keys should still be accessible");
  }
  orc_sdk_hmap_free(map);
}

void test_hmap_remove_after_growth(void)
{
  typedef struct
  {
    int key;
    int value;
  } Entry;
  Entry *map = NULL;
  // Insert elements before growth (assuming ORC_SDK_HMAP_BUCKET_SIZE=8, threshold=75%)
  int pre_growth_keys[]   = {1, 2, 3, 4, 5, 6};
  int pre_growth_values[] = {10, 20, 30, 40, 50, 60};
  for (int i = 0; i < 6; i++) {
    orc_sdk_hmap_put(map, pre_growth_keys[i], pre_growth_values[i]);
  }
  HMapStats stats         = _hmap_stats(map);
  size_t    initial_slots = stats.nslots;
  // Trigger growth
  int growth_keys[]   = {7, 8, 9, 10};
  int growth_values[] = {70, 80, 90, 100};
  for (int i = 0; i < 4; i++) {
    orc_sdk_hmap_put(map, growth_keys[i], growth_values[i]);
  }
  stats = _hmap_stats(map);
  TEST_ASSERT_TRUE_MESSAGE(stats.nslots > initial_slots, "Map should have grown");
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_len(map) == 10,
                           "Should have 10 elements after growth");
  // Remove elements that were inserted before growth
  orc_sdk_hmap_remove(map, pre_growth_keys[1]);  // Remove key 2
  orc_sdk_hmap_remove(map, pre_growth_keys[4]);  // Remove key 5
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_len(map) == 8,
                           "Should have 8 elements after removals");
  TEST_ASSERT_TRUE_MESSAGE(!orc_sdk_hmap_contains(map, pre_growth_keys[1]),
                           "Pre-growth key 2 should be removed");
  TEST_ASSERT_TRUE_MESSAGE(!orc_sdk_hmap_contains(map, pre_growth_keys[4]),
                           "Pre-growth key 5 should be removed");
  // Remove elements that were inserted after growth
  orc_sdk_hmap_remove(map, growth_keys[0]);  // Remove key 7
  orc_sdk_hmap_remove(map, growth_keys[2]);  // Remove key 9
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_len(map) == 6,
                           "Should have 6 elements after more removals");
  TEST_ASSERT_TRUE_MESSAGE(!orc_sdk_hmap_contains(map, growth_keys[0]),
                           "Post-growth key 7 should be removed");
  TEST_ASSERT_TRUE_MESSAGE(!orc_sdk_hmap_contains(map, growth_keys[2]),
                           "Post-growth key 9 should be removed");
  // Verify remaining elements are still accessible
  int remaining_keys[]   = {1, 3, 4, 6, 8, 10};
  int remaining_values[] = {10, 30, 40, 60, 80, 100};
  for (int i = 0; i < 6; i++) {
    TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_contains(map, remaining_keys[i]),
                             "Remaining key should be present");
    Entry *result = (Entry *)orc_sdk_hmap_get(map, remaining_keys[i]);
    TEST_ASSERT_TRUE_MESSAGE(result != NULL, "Should find remaining key");
    TEST_ASSERT_TRUE_MESSAGE(result->value == remaining_values[i],
                             "Remaining key should have correct value");
  }
  orc_sdk_hmap_free(map);
}

void test_hmap_remove_and_reinsert(void)
{
  typedef struct
  {
    int key;
    int value;
  } Entry;
  Entry *map = NULL;
  // Insert initial elements
  int key1 = 10, val1 = 100;
  int key2 = 20, val2 = 200;
  int key3 = 30, val3 = 300;
  orc_sdk_hmap_put(map, key1, val1);
  orc_sdk_hmap_put(map, key2, val2);
  orc_sdk_hmap_put(map, key3, val3);
  HMapStats stats = _hmap_stats(map);
  TEST_ASSERT_TRUE_MESSAGE(stats.n_used == 3, "Should have 3 used slots");
  TEST_ASSERT_TRUE_MESSAGE(stats.n_removed == 0, "Should have 0 removed slots");
  // Remove middle element
  orc_sdk_hmap_remove(map, key2);
  stats = _hmap_stats(map);
  TEST_ASSERT_TRUE_MESSAGE(stats.n_used == 2, "Should have 2 used slots after removal");
  TEST_ASSERT_TRUE_MESSAGE(stats.n_removed <= 1, "Should have at most 1 removed slot");
  TEST_ASSERT_TRUE_MESSAGE(!orc_sdk_hmap_contains(map, key2),
                           "Key2 should not be present");
  // Reinsert the same key with different value
  int new_val2 = 999;
  orc_sdk_hmap_put(map, key2, new_val2);
  stats = _hmap_stats(map);
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_len(map) == 3,
                           "Should have 3 elements after reinsertion");
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_contains(map, key2),
                           "Key2 should be present again");
  Entry *result = (Entry *)orc_sdk_hmap_get(map, key2);
  TEST_ASSERT_TRUE_MESSAGE(result != NULL, "Should find reinserted key");
  TEST_ASSERT_TRUE_MESSAGE(result->value == new_val2,
                           "Reinserted key should have new value");
  // Test multiple remove/reinsert cycles
  for (int cycle = 0; cycle < 3; cycle++) {
    orc_sdk_hmap_remove(map, key1);
    TEST_ASSERT_TRUE_MESSAGE(!orc_sdk_hmap_contains(map, key1),
                             "Key1 should be removed in cycle");
    int cycle_value = 1000 + cycle;
    orc_sdk_hmap_put(map, key1, cycle_value);
    TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_contains(map, key1),
                             "Key1 should be reinserted in cycle");
    result = (Entry *)orc_sdk_hmap_get(map, key1);
    TEST_ASSERT_TRUE_MESSAGE(result != NULL && result->value == cycle_value,
                             "Key1 should have cycle value");
  }
  // Verify other keys remain unaffected
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_contains(map, key3),
                           "Key3 should still be present");
  result = (Entry *)orc_sdk_hmap_get(map, key3);
  TEST_ASSERT_TRUE_MESSAGE(result != NULL && result->value == val3,
                           "Key3 should have original value");
  orc_sdk_hmap_free(map);
}

void test_hmap_remove_null_safety(void)
{
  typedef struct
  {
    int key;
    int value;
  } Entry;
  // Test remove on NULL map
  Entry *null_map = NULL;
  int    test_key = 42;
  orc_sdk_hmap_remove(null_map, test_key);  // Should not crash
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_len(null_map) == 0,
                           "NULL map should remain NULL/empty");
  // Test remove with extreme key values
  Entry *map              = NULL;
  int    extreme_keys[]   = {INT_MAX, INT_MIN, 0, -1};
  int    extreme_values[] = {1000, 2000, 3000, 4000};
  // Insert extreme values
  for (int i = 0; i < 4; i++) {
    orc_sdk_hmap_put(map, extreme_keys[i], extreme_values[i]);
  }
  // Remove extreme values
  for (int i = 0; i < 4; i++) {
    TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_contains(map, extreme_keys[i]),
                             "Extreme key should be present before removal");
    orc_sdk_hmap_remove(map, extreme_keys[i]);
    TEST_ASSERT_TRUE_MESSAGE(!orc_sdk_hmap_contains(map, extreme_keys[i]),
                             "Extreme key should be removed");
  }
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_len(map) == 0,
                           "Map should be empty after removing all extreme values");
  orc_sdk_hmap_free(map);
}

// =============================================================================
// COMPREHENSIVE STRESS TESTS
// =============================================================================

// Test different primitive data types
void test_hmap_data_types_int8(void)
{
  typedef struct
  {
    int8_t key;
    int8_t value;
  } Entry8;
  Entry8 *map = NULL;
  // Test with int8_t range
  for (int8_t i = -50; i < 50; i++) {
    orc_sdk_hmap_put(map, i, (int8_t)(i * 2));
  }
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_len(map) == 100,
                           "Should contain 100 int8 entries");
  // Verify all entries
  for (int8_t i = -50; i < 50; i++) {
    TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_contains(map, i), "Should contain int8 key");
    Entry8 *entry = (Entry8 *)orc_sdk_hmap_get(map, i);
    TEST_ASSERT_TRUE_MESSAGE(entry != NULL && entry->value == i * 2,
                             "Should have correct int8 value");
  }
  orc_sdk_hmap_free(map);
}

void test_hmap_data_types_int64(void)
{
  typedef struct
  {
    int64_t key;
    int64_t value;
  } Entry64;
  Entry64 *map = NULL;
  // Test with large int64_t values
  int64_t test_keys[]   = {INT64_MIN,
                           INT64_MIN + 1,
                           -1000000000000LL,
                           -1,
                           0,
                           1,
                           1000000000000LL,
                           INT64_MAX - 1,
                           INT64_MAX};
  int64_t test_values[] = {1, 2, 3, 4, 5, 6, 7, 8, 9};
  size_t  num_tests     = sizeof(test_keys) / sizeof(test_keys[0]);
  for (size_t i = 0; i < num_tests; i++) {
    orc_sdk_hmap_put(map, test_keys[i], test_values[i]);
  }
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_len(map) == num_tests,
                           "Should contain all int64 entries");
  for (size_t i = 0; i < num_tests; i++) {
    TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_contains(map, test_keys[i]),
                             "Should contain int64 key");
    Entry64 *entry = (Entry64 *)orc_sdk_hmap_get(map, test_keys[i]);
    TEST_ASSERT_TRUE_MESSAGE(entry != NULL && entry->value == test_values[i],
                             "Should have correct int64 value");
  }
  orc_sdk_hmap_free(map);
}

void test_hmap_data_types_float(void)
{
  typedef struct
  {
    float key;
    float value;
  } FloatEntry;
  FloatEntry *map         = NULL;
  float       test_keys[] = {-3.14159f, -1.0f, -0.5f, 0.0f, 0.5f, 1.0f, 2.71828f, 100.5f};
  float       test_values[] = {1.1f, 2.2f, 3.3f, 4.4f, 5.5f, 6.6f, 7.7f, 8.8f};
  size_t      num_tests     = sizeof(test_keys) / sizeof(test_keys[0]);
  for (size_t i = 0; i < num_tests; i++) {
    orc_sdk_hmap_put(map, test_keys[i], test_values[i]);
  }
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_len(map) == num_tests,
                           "Should contain all float entries");
  for (size_t i = 0; i < num_tests; i++) {
    TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_contains(map, test_keys[i]),
                             "Should contain float key");
    FloatEntry *entry = (FloatEntry *)orc_sdk_hmap_get(map, test_keys[i]);
    TEST_ASSERT_TRUE_MESSAGE(entry != NULL && entry->value == test_values[i],
                             "Should have correct float value");
  }
  orc_sdk_hmap_free(map);
}

void test_hmap_data_types_double(void)
{
  typedef struct
  {
    double key;
    double value;
  } DoubleEntry;
  DoubleEntry *map   = NULL;
  double test_keys[] = {-3.141592653589793, -1e-10, 0.0, 1e-10, 2.718281828459045, 1e10};
  double test_values[] = {
    1.123456789, 2.987654321, 3.456789012, 4.321098765, 5.678901234, 6.543210987};
  size_t num_tests = sizeof(test_keys) / sizeof(test_keys[0]);
  for (size_t i = 0; i < num_tests; i++) {
    orc_sdk_hmap_put(map, test_keys[i], test_values[i]);
  }
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_len(map) == num_tests,
                           "Should contain all double entries");
  for (size_t i = 0; i < num_tests; i++) {
    TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_contains(map, test_keys[i]),
                             "Should contain double key");
    DoubleEntry *entry = (DoubleEntry *)orc_sdk_hmap_get(map, test_keys[i]);
    TEST_ASSERT_TRUE_MESSAGE(entry != NULL && entry->value == test_values[i],
                             "Should have correct double value");
  }
  orc_sdk_hmap_free(map);
}

// Compaction stress test - force multiple compactions
void test_hmap_compaction_stress(void)
{
  typedef struct
  {
    int key;
    int value;
  } Entry;
  Entry    *map            = NULL;
  const int NUM_CYCLES     = 5;
  const int KEYS_PER_CYCLE = 100;
  for (int cycle = 0; cycle < NUM_CYCLES; cycle++) {
    // Insert many keys
    for (int i = 0; i < KEYS_PER_CYCLE; i++) {
      int key = cycle * KEYS_PER_CYCLE + i;
      orc_sdk_hmap_put(map, key, key * 10);
    }
    HMapStats stats               = _hmap_stats(map);
    size_t    used_before_removal = stats.n_used;
    // Remove every 3rd key to create tombstones
    for (int i = 0; i < KEYS_PER_CYCLE; i += 3) {
      int key = cycle * KEYS_PER_CYCLE + i;
      orc_sdk_hmap_remove(map, key);
    }
    stats = _hmap_stats(map);
    // Verify correct number of elements remain
    size_t expected_remaining =
      used_before_removal -
      (size_t)(KEYS_PER_CYCLE / 3 + (KEYS_PER_CYCLE % 3 > 0 ? 1 : 0));
    TEST_ASSERT_TRUE_MESSAGE(stats.n_used == expected_remaining,
                             "Should have correct number of used slots after removal");
    // Verify all non-removed keys still exist
    for (int i = 0; i < KEYS_PER_CYCLE; i++) {
      int  key          = cycle * KEYS_PER_CYCLE + i;
      bool should_exist = (i % 3) != 0;
      if (should_exist) {
        TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_contains(map, key),
                                 "Non-removed key should still exist");
        Entry *entry = (Entry *)orc_sdk_hmap_get(map, key);
        TEST_ASSERT_TRUE_MESSAGE(entry != NULL && entry->value == key * 10,
                                 "Should have correct value");
      }
      else {
        TEST_ASSERT_TRUE_MESSAGE(!orc_sdk_hmap_contains(map, key),
                                 "Removed key should not exist");
      }
    }
  }
  orc_sdk_hmap_free(map);
}

// Hash collision stress test - keys designed to have similar hash values
void test_hmap_hash_collision_stress(void)
{
  typedef struct
  {
    int key;
    int value;
  } Entry;
  Entry    *map            = NULL;
  const int NUM_GROUPS     = 20;
  const int KEYS_PER_GROUP = 50;
  // Create keys that are likely to collide by using similar bit patterns
  for (int group = 0; group < NUM_GROUPS; group++) {
    int base_key = group * 1000000;  // Large spacing to create different hash groups
    for (int i = 0; i < KEYS_PER_GROUP; i++) {
      // Create keys with small variations that might hash to same bucket
      int key = base_key + (i * 7) + (i * i);  // Non-linear progression
      orc_sdk_hmap_put(map, key, key + group);
    }
  }
  size_t total_keys = (size_t)(NUM_GROUPS * KEYS_PER_GROUP);
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_len(map) == total_keys,
                           "Should contain all collision test keys");
  // Verify all keys and values
  for (int group = 0; group < NUM_GROUPS; group++) {
    int base_key = group * 1000000;
    for (int i = 0; i < KEYS_PER_GROUP; i++) {
      int key = base_key + (i * 7) + (i * i);
      TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_contains(map, key),
                               "Should contain collision test key");
      Entry *entry = (Entry *)orc_sdk_hmap_get(map, key);
      TEST_ASSERT_TRUE_MESSAGE(entry != NULL && entry->value == key + group,
                               "Should have correct collision test value");
    }
  }
  // Remove half the keys to test collision handling with tombstones
  for (int group = 0; group < NUM_GROUPS; group += 2) {
    int base_key = group * 1000000;
    for (int i = 0; i < KEYS_PER_GROUP; i += 2) {
      int key = base_key + (i * 7) + (i * i);
      orc_sdk_hmap_remove(map, key);
    }
  }
  // Verify remaining keys still work correctly
  for (int group = 0; group < NUM_GROUPS; group++) {
    int  base_key      = group * 1000000;
    bool group_removed = (group % 2 == 0);
    for (int i = 0; i < KEYS_PER_GROUP; i++) {
      int  key              = base_key + (i * 7) + (i * i);
      bool key_should_exist = !group_removed || (i % 2 != 0);
      if (key_should_exist) {
        TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_contains(map, key),
                                 "Remaining collision key should exist");
        Entry *entry = (Entry *)orc_sdk_hmap_get(map, key);
        TEST_ASSERT_TRUE_MESSAGE(entry != NULL && entry->value == key + group,
                                 "Should have correct remaining value");
      }
      else {
        TEST_ASSERT_TRUE_MESSAGE(!orc_sdk_hmap_contains(map, key),
                                 "Removed collision key should not exist");
      }
    }
  }
  orc_sdk_hmap_free(map);
}

// Large-scale performance and correctness test
void test_hmap_large_scale_operations(void)
{
  typedef struct
  {
    int key;
    int value;
  } Entry;
  Entry    *map        = NULL;
  const int LARGE_SIZE = 10000;
  // Phase 1: Insert large number of sequential keys
  for (int i = 0; i < LARGE_SIZE; i++) {
    orc_sdk_hmap_put(map, i, i * 3 + 7);
  }
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_len(map) == (size_t)LARGE_SIZE,
                           "Should contain all large-scale entries");
  HMapStats stats = _hmap_stats(map);
  TEST_ASSERT_TRUE_MESSAGE(stats.n_used == (size_t)LARGE_SIZE,
                           "Should have correct number of used slots");
  TEST_ASSERT_TRUE_MESSAGE(stats.n_removed == 0,
                           "Should have no removed slots after insertion");
  // Phase 2: Verify all entries
  for (int i = 0; i < LARGE_SIZE; i++) {
    TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_contains(map, i),
                             "Should contain large-scale key");
    Entry *entry = (Entry *)orc_sdk_hmap_get(map, i);
    TEST_ASSERT_TRUE_MESSAGE(entry != NULL && entry->value == i * 3 + 7,
                             "Should have correct large-scale value");
  }
  // Phase 3: Remove every 5th element
  int removed_count = 0;
  for (int i = 0; i < LARGE_SIZE; i += 5) {
    orc_sdk_hmap_remove(map, i);
    removed_count++;
  }
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_len(map) == (size_t)(LARGE_SIZE - removed_count),
                           "Should have correct count after large removals");
  // Phase 4: Verify state after removals
  for (int i = 0; i < LARGE_SIZE; i++) {
    bool should_exist = (i % 5) != 0;
    if (should_exist) {
      TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_contains(map, i),
                               "Non-removed large-scale key should exist");
      Entry *entry = (Entry *)orc_sdk_hmap_get(map, i);
      TEST_ASSERT_TRUE_MESSAGE(entry != NULL && entry->value == i * 3 + 7,
                               "Should have correct remaining large-scale value");
    }
    else {
      TEST_ASSERT_TRUE_MESSAGE(!orc_sdk_hmap_contains(map, i),
                               "Removed large-scale key should not exist");
    }
  }
  // Phase 5: Re-insert removed keys with different values
  for (int i = 0; i < LARGE_SIZE; i += 5) {
    orc_sdk_hmap_put(map, i, i * 5 + 11);  // Different value calculation
  }
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_len(map) == (size_t)LARGE_SIZE,
                           "Should be back to full size after re-insertion");
  // Phase 6: Final verification with mixed values
  for (int i = 0; i < LARGE_SIZE; i++) {
    TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_contains(map, i),
                             "Should contain all keys after re-insertion");
    Entry *entry = (Entry *)orc_sdk_hmap_get(map, i);
    TEST_ASSERT_TRUE_MESSAGE(entry != NULL, "Should find all keys after re-insertion");
    int expected_value = (i % 5 == 0) ? (i * 5 + 11) : (i * 3 + 7);
    TEST_ASSERT_TRUE_MESSAGE(entry->value == expected_value,
                             "Should have correct value after mixed operations");
  }
  orc_sdk_hmap_free(map);
}

// Mixed operation patterns test
void test_hmap_mixed_operation_patterns(void)
{
  typedef struct
  {
    int key;
    int value;
  } Entry;
  Entry    *map          = NULL;
  const int PATTERN_SIZE = 1000;
  // Pattern 1: Alternating insert/remove
  for (int i = 0; i < PATTERN_SIZE; i++) {
    orc_sdk_hmap_put(map, i, i * 2);
    if (i > 0 && (i % 3) == 0) {
      int prev_key = i - 2;
      orc_sdk_hmap_remove(map, prev_key);  // Remove an earlier key
    }
  }
  // Pattern 2: Batch operations
  for (int batch = 0; batch < 5; batch++) {
    int batch_start = PATTERN_SIZE + batch * 100;
    // Insert batch
    for (int i = 0; i < 100; i++) {
      int key   = batch_start + i;
      int value = key * 3;
      orc_sdk_hmap_put(map, key, value);
    }
    // Update some values in batch
    for (int i = 0; i < 100; i += 4) {
      int key   = batch_start + i;
      int value = key * 4;  // Different multiplier
      orc_sdk_hmap_put(map, key, value);
    }
    // Remove some from batch
    for (int i = 1; i < 100; i += 4) {
      int key = batch_start + i;
      orc_sdk_hmap_remove(map, key);
    }
  }
  // Pattern 3: Verify the complex state
  size_t final_len = orc_sdk_hmap_len(map);
  TEST_ASSERT_TRUE_MESSAGE(final_len > 0, "Should have elements after mixed operations");
  HMapStats final_stats = _hmap_stats(map);
  TEST_ASSERT_TRUE_MESSAGE(final_stats.n_used == final_len,
                           "Used count should match length");
  // Ensure all contained keys are retrievable and have correct values
  for (int i = 0; i < PATTERN_SIZE + 500; i++) {
    if (orc_sdk_hmap_contains(map, i)) {
      Entry *entry = (Entry *)orc_sdk_hmap_get(map, i);
      TEST_ASSERT_TRUE_MESSAGE(entry != NULL, "Contained key should be retrievable");
      TEST_ASSERT_TRUE_MESSAGE(entry->key == i, "Key should match");
      // Value correctness depends on the complex pattern, just ensure it's not garbage
      TEST_ASSERT_TRUE_MESSAGE(entry->value != 0 || i == 0, "Value should be meaningful");
    }
  }
  orc_sdk_hmap_free(map);
}

void test_hmap_header_alignment(void)
{
  TEST_ASSERT_TRUE_MESSAGE(
    sizeof(_OrcSdk_HashTableHeader) % sizeof(_MaxAlignCompat) == 0,
    "Hashmap header must align with the platform's maximum alignment to be compatible "
    "with arbitrary types inside the container. This doesn't guarantee alignment with "
    "SIMD types. The containers are not meant to be used with SIMD types.");
}

void test_hmap_is_empty(void)
{
  typedef struct
  {
    int key;
    int value;
  } Entry;
  Entry *map = NULL;
  // Test empty map
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_is_empty(map), "NULL map should be empty");
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_len(map) == 0, "NULL map should have length 0");
  // Test after adding element
  int key1 = 42, val1 = 100;
  orc_sdk_hmap_put(map, key1, val1);
  TEST_ASSERT_TRUE_MESSAGE(!orc_sdk_hmap_is_empty(map),
                           "Map with element should not be empty");
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_len(map) == 1, "Map should have length 1");
  // Test after removing element
  orc_sdk_hmap_remove(map, key1);
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_is_empty(map),
                           "Map should be empty after removing only element");
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_len(map) == 0,
                           "Map should have length 0 after removal");
  // Test with multiple elements
  for (int i = 0; i < 10; i++) {
    int key = i, val = i * 2;
    orc_sdk_hmap_put(map, key, val);
    TEST_ASSERT_TRUE_MESSAGE(!orc_sdk_hmap_is_empty(map),
                             "Map should not be empty during population");
  }
  // Remove all elements
  for (int i = 0; i < 10; i++) {
    int key = i;
    orc_sdk_hmap_remove(map, key);
  }
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_is_empty(map),
                           "Map should be empty after removing all elements");
  orc_sdk_hmap_free(map);
}

void test_hmap_iterate_after_remove(void)
{
  typedef struct
  {
    int    key;
    double value;
  } Entry;
  Entry *map = NULL;
  // Test 1: Empty map.
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_is_empty(map), "NULL map should be empty");
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_len(map) == 0, "NULL map should have length 0");
  // Test 2: Insert 10 entries, remove from the middle, check compactness.
  for (int key = 0; key < 10; ++key) {
    orc_sdk_hmap_put(map, key, sin(0.2 * (double)key));
  }
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_len(map) == 10, "Map should have 10 entries");
  {
    HMapStats stats = _hmap_stats(map);
    TEST_ASSERT_TRUE_MESSAGE(stats.n_used == 10,
                             "Should have 10 used slots after insert");
    TEST_ASSERT_TRUE_MESSAGE(stats.n_removed == 0,
                             "No removed slots after fresh inserts");
    TEST_ASSERT_TRUE_MESSAGE(stats.nslots >= 10, "Must have enough slots for 10 entries");
    TEST_ASSERT_TRUE_MESSAGE(stats.nbuckets == stats.nslots / ORC_SDK_HMAP_BUCKET_SIZE,
                             "Bucket count must match slot count");
  }
  for (size_t i = 0; i < orc_sdk_hmap_len(map); ++i) {
    Entry *e = orc_sdk_hmap_get(map, map[i].key);
    TEST_ASSERT_TRUE_MESSAGE(e != NULL && e >= map && e < map + orc_sdk_hmap_len(map),
                             "Every entry must be findable within [0..len)");
  }
  // Test 3: Remove from the middle.
  int remove_key = 3;
  orc_sdk_hmap_remove(map, remove_key);
  TEST_ASSERT_TRUE_MESSAGE(!orc_sdk_hmap_contains(map, remove_key),
                           "Removed key must not be found");
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_len(map) == 9, "Length must decrease");
  {
    HMapStats stats = _hmap_stats(map);
    TEST_ASSERT_TRUE_MESSAGE(stats.n_used == 9,
                             "Should have 9 used slots after removing key 3");
    TEST_ASSERT_TRUE_MESSAGE(stats.n_removed <= 1,
                             "At most 1 tombstone after single remove");
  }
  for (size_t i = 0; i < orc_sdk_hmap_len(map); ++i) {
    Entry *e = orc_sdk_hmap_get(map, map[i].key);
    TEST_ASSERT_TRUE_MESSAGE(e != NULL && e >= map && e < map + orc_sdk_hmap_len(map),
                             "Compact invariant after removing key 3");
  }
  // Test 4: Remove another.
  remove_key = 5;
  orc_sdk_hmap_remove(map, remove_key);
  TEST_ASSERT_TRUE_MESSAGE(!orc_sdk_hmap_contains(map, remove_key),
                           "Removed key must not be found");
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_len(map) == 8, "Length must decrease");
  {
    HMapStats stats = _hmap_stats(map);
    TEST_ASSERT_TRUE_MESSAGE(stats.n_used == 8,
                             "Should have 8 used slots after removing key 5");
    TEST_ASSERT_TRUE_MESSAGE(stats.n_removed <= 2,
                             "At most 2 tombstones after two removes");
  }
  for (size_t i = 0; i < orc_sdk_hmap_len(map); ++i) {
    Entry *e = orc_sdk_hmap_get(map, map[i].key);
    TEST_ASSERT_TRUE_MESSAGE(e != NULL && e >= map && e < map + orc_sdk_hmap_len(map),
                             "Compact invariant after removing key 5");
  }
  // Test 5: Remove the first key (swap with last).
  remove_key = 0;
  orc_sdk_hmap_remove(map, remove_key);
  TEST_ASSERT_TRUE_MESSAGE(!orc_sdk_hmap_contains(map, remove_key),
                           "Removed key 0 must not be found");
  for (size_t i = 0; i < orc_sdk_hmap_len(map); ++i) {
    Entry *e = orc_sdk_hmap_get(map, map[i].key);
    TEST_ASSERT_TRUE_MESSAGE(e != NULL && e >= map && e < map + orc_sdk_hmap_len(map),
                             "Compact invariant after removing key 0");
  }
  // Test 6: Remove the last key (no swap needed).
  remove_key = 9;
  orc_sdk_hmap_remove(map, remove_key);
  TEST_ASSERT_TRUE_MESSAGE(!orc_sdk_hmap_contains(map, remove_key),
                           "Removed key 9 must not be found");
  for (size_t i = 0; i < orc_sdk_hmap_len(map); ++i) {
    Entry *e = orc_sdk_hmap_get(map, map[i].key);
    TEST_ASSERT_TRUE_MESSAGE(e != NULL && e >= map && e < map + orc_sdk_hmap_len(map),
                             "Compact invariant after removing key 9");
  }
  // Test 7: Remove nonexistent key is a no-op.
  {
    size_t const before = orc_sdk_hmap_len(map);
    remove_key          = 999;
    orc_sdk_hmap_remove(map, remove_key);
    TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_len(map) == before,
                             "Removing nonexistent key is a no-op");
  }
  // Test 8: Remove all remaining one by one.
  while (!orc_sdk_hmap_is_empty(map)) {
    int key_to_remove = map[0].key;
    orc_sdk_hmap_remove(map, key_to_remove);
    TEST_ASSERT_TRUE_MESSAGE(!orc_sdk_hmap_contains(map, key_to_remove),
                             "Just-removed key must not exist");
    for (size_t i = 0; i < orc_sdk_hmap_len(map); ++i) {
      Entry *e = orc_sdk_hmap_get(map, map[i].key);
      TEST_ASSERT_TRUE_MESSAGE(e != NULL && e >= map && e < map + orc_sdk_hmap_len(map),
                               "Compact invariant while draining map");
    }
  }
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_len(map) == 0,
                           "Map should be empty after removing all");
  {
    HMapStats stats = _hmap_stats(map);
    TEST_ASSERT_TRUE_MESSAGE(stats.n_used == 0,
                             "Should have 0 used slots when fully drained");
    TEST_ASSERT_TRUE_MESSAGE(stats.n_removed <= 10,
                             "Tombstones bounded by original size (may compact)");
  }
  orc_sdk_hmap_free(map);
  // Test 9: Insert, remove evens, re-insert (exercises swap-remove + growth).
  map = NULL;
  for (int key = 0; key < 50; ++key) {
    orc_sdk_hmap_put(map, key, (double)key);
  }
  for (int key = 0; key < 50; key += 2) {
    orc_sdk_hmap_remove(map, key);
  }
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_len(map) == 25,
                           "25 entries after removing evens");
  {
    HMapStats stats = _hmap_stats(map);
    TEST_ASSERT_TRUE_MESSAGE(stats.n_used == 25,
                             "Should have 25 used slots after removing evens");
    TEST_ASSERT_TRUE_MESSAGE(stats.nslots >= 25, "Must have enough slots for 25 entries");
  }
  for (size_t i = 0; i < orc_sdk_hmap_len(map); ++i) {
    Entry *e = orc_sdk_hmap_get(map, map[i].key);
    TEST_ASSERT_TRUE_MESSAGE(e != NULL && e >= map && e < map + orc_sdk_hmap_len(map),
                             "Compact invariant after removing evens");
  }
  for (int key = 0; key < 50; key += 2) {
    orc_sdk_hmap_put(map, key, (double)(key * 10));
  }
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_len(map) == 50, "50 entries after re-insert");
  {
    HMapStats stats = _hmap_stats(map);
    TEST_ASSERT_TRUE_MESSAGE(stats.n_used == 50,
                             "Should have 50 used slots after re-insert");
    TEST_ASSERT_TRUE_MESSAGE(stats.nslots >= 50, "Must have enough slots for 50 entries");
    TEST_ASSERT_TRUE_MESSAGE(stats.nbuckets == stats.nslots / ORC_SDK_HMAP_BUCKET_SIZE,
                             "Bucket count must match after re-insert");
  }
  for (int key = 0; key < 50; ++key) {
    Entry *e = orc_sdk_hmap_get(map, key);
    TEST_ASSERT_TRUE_MESSAGE(e != NULL, "All 50 keys must exist");
    double expected = (key % 2 == 0) ? (double)(key * 10) : (double)key;
    TEST_ASSERT_TRUE_MESSAGE(e->value == expected, "Value must match after re-insert");
    TEST_ASSERT_TRUE_MESSAGE(e >= map && e < map + orc_sdk_hmap_len(map),
                             "Compact invariant after re-insert");
  }
  orc_sdk_hmap_free(map);
  // Test 10: Many removes triggering compaction (n_removed > n_total/4).
  map = NULL;
  for (int key = 0; key < 100; ++key) {
    orc_sdk_hmap_put(map, key, (double)key);
  }
  for (int key = 0; key < 80; ++key) {
    orc_sdk_hmap_remove(map, key);
    for (size_t i = 0; i < orc_sdk_hmap_len(map); ++i) {
      Entry *e = orc_sdk_hmap_get(map, map[i].key);
      TEST_ASSERT_TRUE_MESSAGE(e != NULL && e >= map && e < map + orc_sdk_hmap_len(map),
                               "Compact invariant during bulk remove");
    }
  }
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_len(map) == 20, "20 entries left");
  {
    HMapStats stats = _hmap_stats(map);
    TEST_ASSERT_TRUE_MESSAGE(stats.n_used == 20,
                             "Should have 20 used slots after bulk remove");
    TEST_ASSERT_TRUE_MESSAGE(stats.nslots >= 20,
                             "Must have enough slots for remaining entries");
  }
  for (size_t i = 0; i < orc_sdk_hmap_len(map); ++i) {
    TEST_ASSERT_TRUE_MESSAGE(map[i].key >= 80 && map[i].key < 100,
                             "Remaining keys must be in [80, 100)");
  }
  orc_sdk_hmap_free(map);
  // Test 11: Single element insert and remove.
  map = NULL;
  {
    int k = 42;
    orc_sdk_hmap_put(map, k, 3.14);
    TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_len(map) == 1, "Single element map");
    Entry *e = orc_sdk_hmap_get(map, k);
    TEST_ASSERT_TRUE_MESSAGE(e != NULL && e >= map && e < map + orc_sdk_hmap_len(map),
                             "Single element compact invariant");
    orc_sdk_hmap_remove(map, k);
    TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hmap_len(map) == 0, "Empty after single remove");
    {
      HMapStats stats = _hmap_stats(map);
      TEST_ASSERT_TRUE_MESSAGE(stats.n_used == 0,
                               "Should have 0 used slots after single remove");
      TEST_ASSERT_TRUE_MESSAGE(stats.n_removed <= 1,
                               "At most 1 tombstone after single remove");
    }
  }
  orc_sdk_hmap_free(map);
}

// Hash Set Tests

void test_hset_basic_operations(void)
{
  int *set = NULL;
  // Test empty set
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hset_len(set) == 0, "New set should be empty");
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hset_is_empty(set), "New set should report as empty");
  // Test adding elements
  int val1 = 42;
  orc_sdk_hset_put(set, val1);
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hset_len(set) == 1, "Set should have 1 element");
  TEST_ASSERT_TRUE_MESSAGE(!orc_sdk_hset_is_empty(set), "Set should not be empty");
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hset_contains(set, val1),
                           "Set should contain added element");
  // Test adding duplicate (should not increase size)
  orc_sdk_hset_put(set, val1);
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hset_len(set) == 1,
                           "Set should still have 1 element after duplicate");
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hset_contains(set, val1),
                           "Set should still contain element");
  // Test adding different elements
  int val2 = 10, val3 = 20, val4 = 30;
  orc_sdk_hset_put(set, val2);
  orc_sdk_hset_put(set, val3);
  orc_sdk_hset_put(set, val4);
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hset_len(set) == 4,
                           "Set should have 4 unique elements");
  // Verify all elements are present
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hset_contains(set, val1), "Set should contain 42");
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hset_contains(set, val2), "Set should contain 10");
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hset_contains(set, val3), "Set should contain 20");
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hset_contains(set, val4), "Set should contain 30");
  // Test non-existent elements
  int missing1 = 99, missing2 = -1;
  TEST_ASSERT_TRUE_MESSAGE(!orc_sdk_hset_contains(set, missing1),
                           "Set should not contain 99");
  TEST_ASSERT_TRUE_MESSAGE(!orc_sdk_hset_contains(set, missing2),
                           "Set should not contain -1");
  orc_sdk_hset_free(set);
}

void test_hset_remove_operations(void)
{
  int *set = NULL;
  // Add some elements
  for (int i = 0; i < 10; i++) {
    orc_sdk_hset_put(set, i);
  }
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hset_len(set) == 10, "Set should have 10 elements");
  // Remove elements
  int remove_val = 5;
  orc_sdk_hset_remove(set, remove_val);
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hset_len(set) == 9,
                           "Set should have 9 elements after removal");
  TEST_ASSERT_TRUE_MESSAGE(!orc_sdk_hset_contains(set, remove_val),
                           "Set should not contain removed element");
  // Remove non-existent element (should not crash or change size)
  int missing_val = 99;
  orc_sdk_hset_remove(set, missing_val);
  TEST_ASSERT_TRUE_MESSAGE(
    orc_sdk_hset_len(set) == 9,
    "Set size should not change when removing non-existent element");
  // Remove all elements
  for (int i = 0; i < 10; i++) {
    if (i != 5) {  // Skip already removed element
      orc_sdk_hset_remove(set, i);
    }
  }
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hset_is_empty(set),
                           "Set should be empty after removing all elements");
  orc_sdk_hset_free(set);
}

void test_hset_different_types(void)
{
  // Test with doubles
  double *dset = NULL;
  double  d1 = 3.14, d2 = 2.71;
  orc_sdk_hset_put(dset, d1);
  orc_sdk_hset_put(dset, d2);
  orc_sdk_hset_put(dset, d1);  // Duplicate
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hset_len(dset) == 2,
                           "Double set should have 2 unique elements");
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hset_contains(dset, d1), "Set should contain 3.14");
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hset_contains(dset, d2), "Set should contain 2.71");
  orc_sdk_hset_free(dset);
  // Test with character values (not strings)
  char *cset = NULL;
  char  c1 = 'a', c2 = 'b', c3 = 'a';  // Duplicate character
  orc_sdk_hset_put(cset, c1);
  orc_sdk_hset_put(cset, c2);
  orc_sdk_hset_put(cset, c3);  // Should not increase size
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hset_len(cset) == 2,
                           "Character set should have 2 unique elements");
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hset_contains(cset, c1), "Set should contain 'a'");
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hset_contains(cset, c2), "Set should contain 'b'");
  orc_sdk_hset_free(cset);
}

void test_hset_large_scale(void)
{
  int      *set  = NULL;
  const int SIZE = 1000;
  // Add many elements
  for (int i = 0; i < SIZE; i++) {
    orc_sdk_hset_put(set, i);
  }
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hset_len(set) == (size_t)SIZE,
                           "Set should contain all added elements");
  // Verify all elements are present
  for (int i = 0; i < SIZE; i++) {
    TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hset_contains(set, i),
                             "Set should contain all added elements");
  }
  // Add duplicates (should not change size)
  for (int i = 0; i < SIZE; i += 2) {
    orc_sdk_hset_put(set, i);
  }
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hset_len(set) == (size_t)SIZE,
                           "Set size should not change after adding duplicates");
  // Remove half the elements
  for (int i = 0; i < SIZE; i += 2) {
    orc_sdk_hset_remove(set, i);
  }
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hset_len(set) == (size_t)SIZE / 2,
                           "Set should have half the elements after removal");
  // Verify correct elements remain
  for (int i = 0; i < SIZE; i++) {
    if (i % 2 == 0) {
      TEST_ASSERT_TRUE_MESSAGE(!orc_sdk_hset_contains(set, i),
                               "Even elements should be removed");
    }
    else {
      TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hset_contains(set, i),
                               "Odd elements should remain");
    }
  }
  orc_sdk_hset_free(set);
}

void test_hset_edge_cases(void)
{
  int *set = NULL;
  // Test operations on NULL set
  int test_val = 42;
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hset_is_empty(set), "NULL set should be empty");
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hset_len(set) == 0, "NULL set should have length 0");
  TEST_ASSERT_TRUE_MESSAGE(!orc_sdk_hset_contains(set, test_val),
                           "NULL set should not contain any elements");
  // Test single element operations
  int single_val = 1;
  orc_sdk_hset_put(set, single_val);
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hset_len(set) == 1, "Set should have 1 element");
  TEST_ASSERT_TRUE_MESSAGE(!orc_sdk_hset_is_empty(set),
                           "Set with element should not be empty");
  orc_sdk_hset_remove(set, single_val);
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hset_is_empty(set),
                           "Set should be empty after removing only element");
  // Test zero value
  int zero_val = 0;
  orc_sdk_hset_put(set, zero_val);
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hset_contains(set, zero_val),
                           "Set should be able to store zero");
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hset_len(set) == 1,
                           "Set with zero should have length 1");
  // Test negative values
  int neg1 = -42, neg2 = -1;
  orc_sdk_hset_put(set, neg1);
  orc_sdk_hset_put(set, neg2);
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hset_contains(set, neg1),
                           "Set should handle negative values");
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hset_contains(set, neg2),
                           "Set should handle negative values");
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hset_len(set) == 3,
                           "Set should have 3 elements (0, -42, -1)");
  orc_sdk_hset_free(set);
}

void test_hset_memory_operations(void)
{
  int *set = NULL;
  // Test reserve functionality
  OrcError result = orc_sdk_hset_reserve(set, 100);
  TEST_ASSERT_TRUE_MESSAGE(result == ORC_ERROR_NONE, "Reserve should succeed");
  // Add elements after reserve
  for (int i = 0; i < 50; i++) {
    orc_sdk_hset_put(set, i);
  }
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hset_len(set) == 50,
                           "Set should have 50 elements after population");
  // Test multiple free calls (should be safe)
  orc_sdk_hset_free(set);
  set = NULL;
  orc_sdk_hset_free(set);  // Should not crash
}

void test_hset_is_empty_comprehensive(void)
{
  int *set = NULL;
  // Test empty states
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hset_is_empty(set), "NULL set should be empty");
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hset_is_empty(NULL), "Explicit NULL should be empty");
  // Test after adding and removing
  int test_elem = 42;
  orc_sdk_hset_put(set, test_elem);
  TEST_ASSERT_TRUE_MESSAGE(!orc_sdk_hset_is_empty(set),
                           "Set with element should not be empty");
  orc_sdk_hset_remove(set, test_elem);
  TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hset_is_empty(set),
                           "Set should be empty after removing only element");
  // Test with multiple add/remove cycles
  for (int cycle = 0; cycle < 5; cycle++) {
    for (int i = 0; i < 10; i++) {
      orc_sdk_hset_put(set, i);
    }
    TEST_ASSERT_TRUE_MESSAGE(!orc_sdk_hset_is_empty(set),
                             "Set should not be empty during population");
    for (int i = 0; i < 10; i++) {
      orc_sdk_hset_remove(set, i);
    }
    TEST_ASSERT_TRUE_MESSAGE(orc_sdk_hset_is_empty(set),
                             "Set should be empty after clearing");
  }
  orc_sdk_hset_free(set);
}

// ============================================================================
// String tests
// ============================================================================

void test_str_null_pointer_operations(void)
{
  char *s = NULL;
  TEST_ASSERT_TRUE_MESSAGE(orc_str_len(s) == 0, "Null string has length 0");
  TEST_ASSERT_TRUE_MESSAGE(orc_str_end(s) == s, "End of NULL string is itself");
  TEST_ASSERT_TRUE_MESSAGE(orc_str_remove(s, 0) == ORC_ERROR_OUT_OF_BOUNDS,
                           "Cannot remove from NULL string");
  // Should not crash
  orc_str_free(s);
}

void test_str_push_basic(void)
{
  char *s = NULL;
  TEST_ASSERT_TRUE(orc_str_push(s, 'h') == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(orc_str_push(s, 'e') == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(orc_str_push(s, 'l') == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(orc_str_push(s, 'l') == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(orc_str_push(s, 'o') == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE_MESSAGE(orc_str_len(s) == 5, "String length should be 5");
  TEST_ASSERT_TRUE_MESSAGE(strcmp(s, "hello") == 0, "String content should be 'hello'");
  TEST_ASSERT_TRUE_MESSAGE(s[orc_str_len(s)] == '\0', "String must be null-terminated");
  orc_str_free(s);
}

void test_str_push_from_null(void)
{
  char *s = NULL;
  // First push allocates
  TEST_ASSERT_TRUE(orc_str_push(s, 'a') == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(s != NULL);
  TEST_ASSERT_TRUE(orc_str_len(s) == 1);
  TEST_ASSERT_TRUE(s[0] == 'a');
  TEST_ASSERT_TRUE(s[1] == '\0');
  // Subsequent pushes
  TEST_ASSERT_TRUE(orc_str_push(s, 'b') == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(orc_str_len(s) == 2);
  TEST_ASSERT_TRUE(strcmp(s, "ab") == 0);
  TEST_ASSERT_TRUE(orc_str_push(s, 'c') == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(orc_str_len(s) == 3);
  TEST_ASSERT_TRUE(strcmp(s, "abc") == 0);
  orc_str_free(s);
}

void test_orc_str_remove_basic(void)
{
  char *s = NULL;
  // Build "abcde"
  TEST_ASSERT_TRUE(orc_str_push(s, 'a') == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(orc_str_push(s, 'b') == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(orc_str_push(s, 'c') == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(orc_str_push(s, 'd') == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(orc_str_push(s, 'e') == ORC_ERROR_NONE);
  // Remove middle character 'c' at index 2
  TEST_ASSERT_TRUE(orc_str_remove(s, 2) == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(orc_str_len(s) == 4);
  TEST_ASSERT_TRUE(strcmp(s, "abde") == 0);
  TEST_ASSERT_TRUE(s[orc_str_len(s)] == '\0');
  // Remove first character 'a' at index 0
  TEST_ASSERT_TRUE(orc_str_remove(s, 0) == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(orc_str_len(s) == 3);
  TEST_ASSERT_TRUE(strcmp(s, "bde") == 0);
  TEST_ASSERT_TRUE(s[orc_str_len(s)] == '\0');
  // Remove last character 'e' at index 2
  TEST_ASSERT_TRUE(orc_str_remove(s, orc_str_len(s) - 1) == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(orc_str_len(s) == 2);
  TEST_ASSERT_TRUE(strcmp(s, "bd") == 0);
  TEST_ASSERT_TRUE(s[orc_str_len(s)] == '\0');
  orc_str_free(s);
}

void test_orc_str_remove_boundary_conditions(void)
{
  // Single character string
  char *s = NULL;
  TEST_ASSERT_TRUE(orc_str_push(s, 'x') == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(orc_str_remove(s, 0) == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE_MESSAGE(orc_str_len(s) == 0, "Empty after removing only character");
  TEST_ASSERT_TRUE_MESSAGE(s[0] == '\0', "Still null-terminated when empty");
  // Remove from empty (non-NULL) string
  TEST_ASSERT_TRUE_MESSAGE(orc_str_remove(s, 0) == ORC_ERROR_OUT_OF_BOUNDS,
                           "Cannot remove from empty string");
  // One past end
  TEST_ASSERT_TRUE(orc_str_push(s, 'a') == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(orc_str_remove(s, orc_str_len(s)) == ORC_ERROR_OUT_OF_BOUNDS);
  // Way past end
  TEST_ASSERT_TRUE(orc_str_remove(s, orc_str_len(s) + 10) == ORC_ERROR_OUT_OF_BOUNDS);
  // Huge index
  TEST_ASSERT_TRUE(orc_str_remove(s, SIZE_MAX) == ORC_ERROR_OUT_OF_BOUNDS);
  // Remove from NULL
  char *null_str = NULL;
  TEST_ASSERT_TRUE(orc_str_remove(null_str, 0) == ORC_ERROR_OUT_OF_BOUNDS);
  orc_str_free(s);
}

void test_orc_str_len_and_end(void)
{
  char *s = NULL;
  TEST_ASSERT_TRUE(orc_str_len(s) == 0);
  // Build "abc"
  TEST_ASSERT_TRUE(orc_str_push(s, 'a') == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(orc_str_push(s, 'b') == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(orc_str_push(s, 'c') == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(orc_str_len(s) == 3);
  TEST_ASSERT_TRUE(orc_str_end(s) == s + orc_str_len(s));
  TEST_ASSERT_TRUE(*orc_str_end(s) == '\0');
  // After removal
  TEST_ASSERT_TRUE(orc_str_remove(s, 1) == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(orc_str_len(s) == 2);
  TEST_ASSERT_TRUE(orc_str_end(s) == s + orc_str_len(s));
  TEST_ASSERT_TRUE(*orc_str_end(s) == '\0');
  orc_str_free(s);
}

void test_orc_str_free_and_reuse(void)
{
  char *s = NULL;
  TEST_ASSERT_TRUE(orc_str_push(s, 'a') == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(orc_str_push(s, 'b') == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(orc_str_len(s) == 2);
  orc_str_free(s);
  TEST_ASSERT_TRUE_MESSAGE(s == NULL, "Pointer is NULL after free");
  // Reuse after free
  TEST_ASSERT_TRUE(orc_str_push(s, 'x') == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(s != NULL);
  TEST_ASSERT_TRUE(orc_str_len(s) == 1);
  TEST_ASSERT_TRUE(strcmp(s, "x") == 0);
  orc_str_free(s);
}

void test_str_push_special_characters(void)
{
  char *s = NULL;
  // Whitespace characters
  TEST_ASSERT_TRUE(orc_str_push(s, ' ') == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(orc_str_push(s, '\t') == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(orc_str_push(s, '\n') == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(orc_str_len(s) == 3);
  TEST_ASSERT_TRUE(s[0] == ' ');
  TEST_ASSERT_TRUE(s[1] == '\t');
  TEST_ASSERT_TRUE(s[2] == '\n');
  TEST_ASSERT_TRUE(s[3] == '\0');
  orc_str_free(s);
  // Non-ASCII / high bytes
  s = NULL;
  TEST_ASSERT_TRUE(orc_str_push(s, (char)0xFF) == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(orc_str_push(s, (char)0x80) == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(orc_str_push(s, (char)0x01) == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(orc_str_len(s) == 3);
  TEST_ASSERT_TRUE(s[0] == (char)0xFF);
  TEST_ASSERT_TRUE(s[1] == (char)0x80);
  TEST_ASSERT_TRUE(s[2] == (char)0x01);
  TEST_ASSERT_TRUE(s[3] == '\0');
  orc_str_free(s);
  // Pushing a null byte - the header count still grows,
  // so orc_str_len reports based on header, not C string length
  s = NULL;
  TEST_ASSERT_TRUE(orc_str_push(s, 'a') == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(orc_str_push(s, '\0') == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(orc_str_push(s, 'b') == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE_MESSAGE(orc_str_len(s) == 3,
                           "orc_str_len tracks header count, not strlen");
  // But strlen would see only 1
  TEST_ASSERT_TRUE_MESSAGE(strlen(s) == 1, "C strlen stops at embedded null");
  orc_str_free(s);
}

void test_str_capacity_growth(void)
{
  char        *s = NULL;
  size_t const n = 256;
  for (size_t i = 0; i < n; i++) {
    char ch = (char)('a' + (char)(i % 26));
    TEST_ASSERT_TRUE(orc_str_push(s, ch) == ORC_ERROR_NONE);
    TEST_ASSERT_TRUE(orc_str_len(s) == i + 1);
    TEST_ASSERT_TRUE(s[orc_str_len(s)] == '\0');
    // Capacity must be at least length + 1 (for null terminator)
    TEST_ASSERT_TRUE(_orc_sdk_arr_capacity(s) >= orc_str_len(s) + 1);
  }
  // Verify final content
  for (size_t i = 0; i < n; i++) {
    char expected = (char)('a' + (char)(i % 26));
    TEST_ASSERT_TRUE(s[i] == expected);
  }
  TEST_ASSERT_TRUE(orc_str_len(s) == n);
  orc_str_free(s);
}

void test_str_mixed_operations(void)
{
  char *s = NULL;
  // Build "hello"
  TEST_ASSERT_TRUE(orc_str_push(s, 'h') == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(orc_str_push(s, 'e') == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(orc_str_push(s, 'l') == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(orc_str_push(s, 'l') == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(orc_str_push(s, 'o') == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(strcmp(s, "hello") == 0);
  // Remove 'e' at index 1 -> "hllo"
  TEST_ASSERT_TRUE(orc_str_remove(s, 1) == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(strcmp(s, "hllo") == 0);
  TEST_ASSERT_TRUE(s[orc_str_len(s)] == '\0');
  // Push 'e' -> "hlloe"
  TEST_ASSERT_TRUE(orc_str_push(s, 'e') == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(strcmp(s, "hlloe") == 0);
  TEST_ASSERT_TRUE(s[orc_str_len(s)] == '\0');
  // Remove all characters one by one from front
  size_t len = orc_str_len(s);
  for (size_t i = 0; i < len; i++) {
    TEST_ASSERT_TRUE(orc_str_remove(s, 0) == ORC_ERROR_NONE);
  }
  TEST_ASSERT_TRUE(orc_str_len(s) == 0);
  TEST_ASSERT_TRUE(s[0] == '\0');
  // Push again after emptying
  TEST_ASSERT_TRUE(orc_str_push(s, 'z') == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(orc_str_len(s) == 1);
  TEST_ASSERT_TRUE(strcmp(s, "z") == 0);
  orc_str_free(s);
}

void test_str_single_character(void)
{
  char *s = NULL;
  TEST_ASSERT_TRUE(orc_str_push(s, 'x') == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(orc_str_len(s) == 1);
  TEST_ASSERT_TRUE(s[0] == 'x');
  TEST_ASSERT_TRUE(s[1] == '\0');
  TEST_ASSERT_TRUE(orc_str_end(s) == s + 1);
  // Remove it
  TEST_ASSERT_TRUE(orc_str_remove(s, 0) == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(orc_str_len(s) == 0);
  TEST_ASSERT_TRUE(s[0] == '\0');
  TEST_ASSERT_TRUE(orc_str_end(s) == s);
  // Push again - recover from empty
  TEST_ASSERT_TRUE(orc_str_push(s, 'y') == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(orc_str_len(s) == 1);
  TEST_ASSERT_TRUE(s[0] == 'y');
  TEST_ASSERT_TRUE(s[1] == '\0');
  orc_str_free(s);
}

void test_str_long_string(void)
{
  char        *s = NULL;
  size_t const n = 10000;
  // Build a long string
  for (size_t i = 0; i < n; i++) {
    TEST_ASSERT_TRUE(orc_str_push(s, (char)('A' + (char)(i % 26))) == ORC_ERROR_NONE);
  }
  TEST_ASSERT_TRUE(orc_str_len(s) == n);
  TEST_ASSERT_TRUE(s[n] == '\0');
  // Verify content
  for (size_t i = 0; i < n; i++) {
    TEST_ASSERT_TRUE(s[i] == (char)('A' + (char)(i % 26)));
  }
  // Remove 100 characters from the front
  for (size_t i = 0; i < 100; i++) {
    TEST_ASSERT_TRUE(orc_str_remove(s, 0) == ORC_ERROR_NONE);
  }
  TEST_ASSERT_TRUE(orc_str_len(s) == n - 100);
  TEST_ASSERT_TRUE(s[orc_str_len(s)] == '\0');
  // Remove from the end
  for (size_t i = 0; i < 100; i++) {
    TEST_ASSERT_TRUE(orc_str_remove(s, orc_str_len(s) - 1) == ORC_ERROR_NONE);
  }
  TEST_ASSERT_TRUE(orc_str_len(s) == n - 200);
  TEST_ASSERT_TRUE(s[orc_str_len(s)] == '\0');
  // Remove from middle
  size_t mid = orc_str_len(s) / 2;
  TEST_ASSERT_TRUE(orc_str_remove(s, mid) == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(orc_str_len(s) == n - 201);
  TEST_ASSERT_TRUE(s[orc_str_len(s)] == '\0');
  orc_str_free(s);
}

// orc_str_clear tests

void test_orc_str_clear_basic(void)
{
  char *s = NULL;
  TEST_ASSERT_TRUE(orc_str_push(s, 'a') == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(orc_str_push(s, 'b') == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(orc_str_push(s, 'c') == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(orc_str_len(s) == 3);
  orc_str_clear(s);
  TEST_ASSERT_TRUE_MESSAGE(orc_str_len(s) == 0, "Length is 0 after clear");
  TEST_ASSERT_TRUE_MESSAGE(s[0] == '\0', "Null-terminated after clear");
  TEST_ASSERT_TRUE_MESSAGE(orc_str_is_empty(s), "String is empty after clear");
  // Capacity should be preserved
  TEST_ASSERT_TRUE(_orc_sdk_arr_capacity(s) >= 1);
  orc_str_free(s);
}

void test_orc_str_clear_null(void)
{
  // Should not crash
  char *s = NULL;
  orc_str_clear(s);
  TEST_ASSERT_TRUE(s == NULL);
}

void test_orc_str_clear_and_reuse(void)
{
  char *s = NULL;
  TEST_ASSERT_TRUE(orc_str_push(s, 'x') == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(orc_str_push(s, 'y') == ORC_ERROR_NONE);
  orc_str_clear(s);
  TEST_ASSERT_TRUE(orc_str_len(s) == 0);
  // Push after clear
  TEST_ASSERT_TRUE(orc_str_push(s, 'a') == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(orc_str_len(s) == 1);
  TEST_ASSERT_TRUE(strcmp(s, "a") == 0);
  // Clear and push again
  orc_str_clear(s);
  TEST_ASSERT_TRUE(orc_str_push(s, 'b') == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(orc_str_push(s, 'c') == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(strcmp(s, "bc") == 0);
  orc_str_free(s);
}

void test_orc_str_clear_already_empty(void)
{
  char *s = NULL;
  TEST_ASSERT_TRUE(orc_str_push(s, 'a') == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(orc_str_remove(s, 0) == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(orc_str_len(s) == 0);
  // Clear an already-empty (but allocated) string
  orc_str_clear(s);
  TEST_ASSERT_TRUE(orc_str_len(s) == 0);
  TEST_ASSERT_TRUE(s[0] == '\0');
  orc_str_free(s);
}

// orc_str_push_str tests

void test_str_push_str_basic(void)
{
  char *s = NULL;
  TEST_ASSERT_TRUE(orc_str_push(s, 'h') == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(orc_str_push(s, 'i') == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(orc_str_push_str(s, " world") == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE_MESSAGE(orc_str_len(s) == 8, "Length after push_str");
  TEST_ASSERT_TRUE_MESSAGE(strcmp(s, "hi world") == 0, "Content after push_str");
  TEST_ASSERT_TRUE(s[orc_str_len(s)] == '\0');
  orc_str_free(s);
}

void test_str_push_str_to_null(void)
{
  // Push string onto NULL pointer
  char *s = NULL;
  TEST_ASSERT_TRUE(orc_str_push_str(s, "hello") == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(s != NULL);
  TEST_ASSERT_TRUE(orc_str_len(s) == 5);
  TEST_ASSERT_TRUE(strcmp(s, "hello") == 0);
  TEST_ASSERT_TRUE(s[orc_str_len(s)] == '\0');
  orc_str_free(s);
}

void test_str_push_str_empty_tail(void)
{
  // Push empty string onto existing string
  char *s = NULL;
  TEST_ASSERT_TRUE(orc_str_push_str(s, "abc") == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(orc_str_push_str(s, "") == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE_MESSAGE(orc_str_len(s) == 3,
                           "Length unchanged after pushing empty string");
  TEST_ASSERT_TRUE(strcmp(s, "abc") == 0);
  orc_str_free(s);
}

void test_str_push_str_empty_tail_to_null(void)
{
  // Push empty string onto NULL - should allocate an empty string, not fail
  char *s = NULL;
  TEST_ASSERT_TRUE_MESSAGE(orc_str_push_str(s, "") == ORC_ERROR_NONE,
                           "Pushing empty to NULL should succeed");
  TEST_ASSERT_TRUE(s != NULL);
  TEST_ASSERT_TRUE(orc_str_len(s) == 0);
  TEST_ASSERT_TRUE(s[0] == '\0');
  orc_str_free(s);
}

void test_str_push_str_multiple(void)
{
  char *s = NULL;
  TEST_ASSERT_TRUE(orc_str_push_str(s, "foo") == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(orc_str_push_str(s, "bar") == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(orc_str_push_str(s, "baz") == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(orc_str_len(s) == 9);
  TEST_ASSERT_TRUE(strcmp(s, "foobarbaz") == 0);
  TEST_ASSERT_TRUE(s[orc_str_len(s)] == '\0');
  orc_str_free(s);
}

void test_str_push_str_after_remove(void)
{
  char *s = NULL;
  TEST_ASSERT_TRUE(orc_str_push_str(s, "abcde") == ORC_ERROR_NONE);
  // Remove middle character
  TEST_ASSERT_TRUE(orc_str_remove(s, 2) == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(strcmp(s, "abde") == 0);
  // Push string after removal
  TEST_ASSERT_TRUE(orc_str_push_str(s, "XY") == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(strcmp(s, "abdeXY") == 0);
  TEST_ASSERT_TRUE(orc_str_len(s) == 6);
  TEST_ASSERT_TRUE(s[orc_str_len(s)] == '\0');
  orc_str_free(s);
}

void test_str_push_str_after_clear(void)
{
  char *s = NULL;
  TEST_ASSERT_TRUE(orc_str_push_str(s, "hello") == ORC_ERROR_NONE);
  orc_str_clear(s);
  TEST_ASSERT_TRUE(orc_str_len(s) == 0);
  TEST_ASSERT_TRUE(orc_str_push_str(s, "world") == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(strcmp(s, "world") == 0);
  TEST_ASSERT_TRUE(orc_str_len(s) == 5);
  orc_str_free(s);
}

void test_str_push_str_long(void)
{
  char *s = NULL;
  // Build a long string by appending many times
  for (int i = 0; i < 500; i++) {
    TEST_ASSERT_TRUE(orc_str_push_str(s, "ab") == ORC_ERROR_NONE);
  }
  TEST_ASSERT_TRUE(orc_str_len(s) == 1000);
  TEST_ASSERT_TRUE(s[orc_str_len(s)] == '\0');
  // Verify pattern
  for (size_t i = 0; i < 1000; i += 2) {
    TEST_ASSERT_TRUE(s[i] == 'a');
    TEST_ASSERT_TRUE(s[i + 1] == 'b');
  }
  orc_str_free(s);
}

void test_str_push_str_single_char(void)
{
  // Push a single-character string (compare behavior with orc_str_push)
  char *s = NULL;
  TEST_ASSERT_TRUE(orc_str_push_str(s, "x") == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(orc_str_len(s) == 1);
  TEST_ASSERT_TRUE(strcmp(s, "x") == 0);
  // Equivalent to orc_str_push
  char *s2 = NULL;
  TEST_ASSERT_TRUE(orc_str_push(s2, 'x') == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(orc_str_len(s2) == 1);
  TEST_ASSERT_TRUE(strcmp(s, s2) == 0);
  orc_str_free(s);
  orc_str_free(s2);
}

// orc_str_is_empty tests

void test_orc_str_is_empty_null(void)
{
  char *s = NULL;
  TEST_ASSERT_TRUE_MESSAGE(orc_str_is_empty(s), "NULL string is empty");
}

void test_orc_str_is_empty_after_operations(void)
{
  char *s = NULL;
  TEST_ASSERT_TRUE(orc_str_is_empty(s));
  TEST_ASSERT_TRUE(orc_str_push(s, 'a') == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE_MESSAGE(!orc_str_is_empty(s), "Non-empty after push");
  TEST_ASSERT_TRUE(orc_str_remove(s, 0) == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE_MESSAGE(orc_str_is_empty(s), "Empty after removing last char");
  TEST_ASSERT_TRUE(orc_str_push_str(s, "hi") == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(!orc_str_is_empty(s));
  orc_str_clear(s);
  TEST_ASSERT_TRUE_MESSAGE(orc_str_is_empty(s), "Empty after clear");
  orc_str_free(s);
}

// Mixed operations across new and old API

void test_str_mixed_new_operations(void)
{
  char *s = NULL;
  TEST_ASSERT_TRUE(orc_str_push_str(s, "hello") == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(orc_str_push(s, '!') == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(strcmp(s, "hello!") == 0);
  orc_str_clear(s);
  TEST_ASSERT_TRUE(orc_str_is_empty(s));
  TEST_ASSERT_TRUE(orc_str_push(s, 'A') == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(orc_str_push_str(s, "BC") == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(strcmp(s, "ABC") == 0);
  TEST_ASSERT_TRUE(orc_str_remove(s, 1) == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(strcmp(s, "AC") == 0);
  TEST_ASSERT_TRUE(orc_str_push_str(s, "DE") == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(strcmp(s, "ACDE") == 0);
  TEST_ASSERT_TRUE(orc_str_len(s) == 4);
  TEST_ASSERT_TRUE(s[orc_str_len(s)] == '\0');
  orc_str_free(s);
}

// String view.

void test_orc_sv_from_str_and_basics(void)
{
  // From a normal string
  char       buf[] = "hello";
  OrcStrView sv    = orc_sv_from_str(buf);
  TEST_ASSERT_TRUE(sv.start == buf);
  TEST_ASSERT_TRUE(sv.end == buf + 5);
  TEST_ASSERT_TRUE(orc_sv_len(sv) == 5);
  TEST_ASSERT_TRUE(!orc_sv_is_empty(sv));
  // From empty string
  char       empty[] = "";
  OrcStrView e       = orc_sv_from_str(empty);
  TEST_ASSERT_TRUE(e.start == empty);
  TEST_ASSERT_TRUE(e.end == empty);
  TEST_ASSERT_TRUE(orc_sv_len(e) == 0);
  TEST_ASSERT_TRUE(orc_sv_is_empty(e));
  // From NULL
  OrcStrView n = orc_sv_from_str(NULL);
  TEST_ASSERT_TRUE(n.start == NULL);
  TEST_ASSERT_TRUE(n.end == NULL);
  TEST_ASSERT_TRUE(orc_sv_len(n) == 0);
  TEST_ASSERT_TRUE(orc_sv_is_empty(n));
}

void test_orc_sv_trim(void)
{
  // Trim left
  char       buf1[] = "  hi";
  OrcStrView sv1    = orc_sv_trim_left(orc_sv_from_str(buf1));
  TEST_ASSERT_TRUE(orc_sv_len(sv1) == 2);
  TEST_ASSERT_TRUE(memcmp(sv1.start, "hi", 2) == 0);
  // Trim right
  char       buf2[] = "hi  ";
  OrcStrView sv2    = orc_sv_trim_right(orc_sv_from_str(buf2));
  TEST_ASSERT_TRUE(orc_sv_len(sv2) == 2);
  TEST_ASSERT_TRUE(memcmp(sv2.start, "hi", 2) == 0);
  // Trim both
  char       buf3[] = " \t hi \n ";
  OrcStrView sv3    = orc_sv_trim_right(orc_sv_trim_left(orc_sv_from_str(buf3)));
  TEST_ASSERT_TRUE(orc_sv_len(sv3) == 2);
  TEST_ASSERT_TRUE(memcmp(sv3.start, "hi", 2) == 0);
  // All whitespace trims to empty
  char       buf4[] = "   ";
  OrcStrView sv4    = orc_sv_trim_left(orc_sv_from_str(buf4));
  TEST_ASSERT_TRUE(orc_sv_is_empty(sv4));
  char       buf5[] = "   ";
  OrcStrView sv5    = orc_sv_trim_right(orc_sv_from_str(buf5));
  TEST_ASSERT_TRUE(orc_sv_is_empty(sv5));
  // No whitespace is a no-op
  char       buf6[] = "abc";
  OrcStrView sv6    = orc_sv_trim_left(orc_sv_trim_right(orc_sv_from_str(buf6)));
  TEST_ASSERT_TRUE(orc_sv_len(sv6) == 3);
  TEST_ASSERT_TRUE(memcmp(sv6.start, "abc", 3) == 0);
  // Empty view
  OrcStrView sv7 = orc_sv_trim_left(orc_sv_from_str(""));
  TEST_ASSERT_TRUE(orc_sv_is_empty(sv7));
  OrcStrView sv8 = orc_sv_trim_right(orc_sv_from_str(""));
  TEST_ASSERT_TRUE(orc_sv_is_empty(sv8));
  // NULL view
  OrcStrView null_sv = orc_sv_trim_left((OrcStrView) {0});
  TEST_ASSERT_TRUE(null_sv.start == NULL);
  TEST_ASSERT_TRUE(null_sv.end == NULL);
  null_sv = orc_sv_trim_right((OrcStrView) {0});
  TEST_ASSERT_TRUE(null_sv.start == NULL);
  TEST_ASSERT_TRUE(null_sv.end == NULL);
}

void test_orc_sv_split_at_delim(void)
{
  // Basic split on comma
  char       buf[] = "one,two,three";
  OrcStrView sv    = orc_sv_from_str(buf);
  OrcStrView part1 = orc_sv_split_at_delim(&sv, ',');
  TEST_ASSERT_TRUE(orc_sv_len(part1) == 3);
  TEST_ASSERT_TRUE(memcmp(part1.start, "one", 3) == 0);
  TEST_ASSERT_TRUE_MESSAGE(sv.start == buf + 4, "Remainder starts after delimiter");
  OrcStrView part2 = orc_sv_split_at_delim(&sv, ',');
  TEST_ASSERT_TRUE(orc_sv_len(part2) == 3);
  TEST_ASSERT_TRUE(memcmp(part2.start, "two", 3) == 0);
  // Last segment — no more delimiters, returns remainder and nulls out sv
  OrcStrView part3 = orc_sv_split_at_delim(&sv, ',');
  TEST_ASSERT_TRUE(orc_sv_len(part3) == 5);
  TEST_ASSERT_TRUE(memcmp(part3.start, "three", 5) == 0);
  TEST_ASSERT_TRUE(sv.start == NULL);
  TEST_ASSERT_TRUE(sv.end == NULL);
  // Splitting an exhausted view returns empty
  OrcStrView part4 = orc_sv_split_at_delim(&sv, ',');
  TEST_ASSERT_TRUE(orc_sv_is_empty(part4));
  // Delimiter at start yields empty first part
  char       buf2[] = ",hello";
  OrcStrView sv2    = orc_sv_from_str(buf2);
  OrcStrView first  = orc_sv_split_at_delim(&sv2, ',');
  TEST_ASSERT_TRUE(orc_sv_len(first) == 0);
  TEST_ASSERT_TRUE(orc_sv_len(sv2) == 5);
  TEST_ASSERT_TRUE(memcmp(sv2.start, "hello", 5) == 0);
  // Delimiter at end yields content then empty
  char       buf3[] = "hello,";
  OrcStrView sv3    = orc_sv_from_str(buf3);
  OrcStrView before = orc_sv_split_at_delim(&sv3, ',');
  TEST_ASSERT_TRUE(orc_sv_len(before) == 5);
  TEST_ASSERT_TRUE(memcmp(before.start, "hello", 5) == 0);
  OrcStrView after = orc_sv_split_at_delim(&sv3, ',');
  TEST_ASSERT_TRUE(orc_sv_len(after) == 0);
  TEST_ASSERT_TRUE(sv3.start == NULL);
  // No delimiter at all
  char       buf4[] = "none";
  OrcStrView sv4    = orc_sv_from_str(buf4);
  OrcStrView whole  = orc_sv_split_at_delim(&sv4, ',');
  TEST_ASSERT_TRUE(orc_sv_len(whole) == 4);
  TEST_ASSERT_TRUE(memcmp(whole.start, "none", 4) == 0);
  TEST_ASSERT_TRUE(sv4.start == NULL);
}

void test_orc_sv_split_line(void)
{
  char       buf[] = "line1\nline2\nline3";
  OrcStrView sv    = orc_sv_from_str(buf);
  OrcStrView l1    = orc_sv_split_line(&sv);
  TEST_ASSERT_TRUE(orc_sv_len(l1) == 5);
  TEST_ASSERT_TRUE(memcmp(l1.start, "line1", 5) == 0);
  OrcStrView l2 = orc_sv_split_line(&sv);
  TEST_ASSERT_TRUE(orc_sv_len(l2) == 5);
  TEST_ASSERT_TRUE(memcmp(l2.start, "line2", 5) == 0);
  OrcStrView l3 = orc_sv_split_line(&sv);
  TEST_ASSERT_TRUE(orc_sv_len(l3) == 5);
  TEST_ASSERT_TRUE(memcmp(l3.start, "line3", 5) == 0);
  TEST_ASSERT_TRUE(sv.start == NULL);
}

void test_sv_trim_combined(void)
{
  char       buf1[] = " \t hello \n ";
  OrcStrView sv1    = orc_sv_trim(orc_sv_from_str(buf1));
  TEST_ASSERT_TRUE(orc_sv_len(sv1) == 5);
  TEST_ASSERT_TRUE(memcmp(sv1.start, "hello", 5) == 0);
  // No whitespace
  char       buf2[] = "abc";
  OrcStrView sv2    = orc_sv_trim(orc_sv_from_str(buf2));
  TEST_ASSERT_TRUE(orc_sv_len(sv2) == 3);
  TEST_ASSERT_TRUE(memcmp(sv2.start, "abc", 3) == 0);
  // All whitespace
  char       buf3[] = "   ";
  OrcStrView sv3    = orc_sv_trim(orc_sv_from_str(buf3));
  TEST_ASSERT_TRUE(orc_sv_is_empty(sv3));
  // Empty and NULL
  TEST_ASSERT_TRUE(orc_sv_is_empty(orc_sv_trim(orc_sv_from_str(""))));
  TEST_ASSERT_TRUE(orc_sv_trim((OrcStrView) {0}).start == NULL);
}

void test_orc_sv_starts_with(void)
{
  char       buf[] = "hello world";
  OrcStrView sv    = orc_sv_from_str(buf);
  TEST_ASSERT_TRUE(orc_sv_starts_with(sv, "hello"));
  TEST_ASSERT_TRUE(orc_sv_starts_with(sv, "h"));
  TEST_ASSERT_TRUE(orc_sv_starts_with(sv, "hello world"));
  TEST_ASSERT_TRUE(!orc_sv_starts_with(sv, "hello world!"));
  TEST_ASSERT_TRUE(!orc_sv_starts_with(sv, "world"));
  TEST_ASSERT_TRUE(!orc_sv_starts_with(sv, "Hello"));
  // NULL prefix
  TEST_ASSERT_TRUE(!orc_sv_starts_with(sv, NULL));
  // Empty view
  OrcStrView empty = orc_sv_from_str("");
  TEST_ASSERT_TRUE(!orc_sv_starts_with(empty, "a"));
  // NULL view
  OrcStrView null_sv = (OrcStrView) {0};
  TEST_ASSERT_TRUE(!orc_sv_starts_with(null_sv, "a"));
  // Single char view
  char       buf2[] = "x";
  OrcStrView sv2    = orc_sv_from_str(buf2);
  TEST_ASSERT_TRUE(orc_sv_starts_with(sv2, "x"));
  TEST_ASSERT_TRUE(!orc_sv_starts_with(sv2, "xy"));
}

void test_orc_sv_ends_with(void)
{
  char       buf[] = "hello world";
  OrcStrView sv    = orc_sv_from_str(buf);
  TEST_ASSERT_TRUE(orc_sv_ends_with(sv, "world"));
  TEST_ASSERT_TRUE(orc_sv_ends_with(sv, "d"));
  TEST_ASSERT_TRUE(orc_sv_ends_with(sv, "hello world"));
  TEST_ASSERT_TRUE(!orc_sv_ends_with(sv, "hello world!"));
  TEST_ASSERT_TRUE(!orc_sv_ends_with(sv, "hello"));
  TEST_ASSERT_TRUE(!orc_sv_ends_with(sv, "World"));
  // NULL suffix
  TEST_ASSERT_TRUE(!orc_sv_ends_with(sv, NULL));
  // Empty view
  OrcStrView empty = orc_sv_from_str("");
  TEST_ASSERT_TRUE(!orc_sv_ends_with(empty, "a"));
  // NULL view
  OrcStrView null_sv = (OrcStrView) {0};
  TEST_ASSERT_TRUE(!orc_sv_ends_with(null_sv, "a"));
  // Single char view
  char       buf2[] = "x";
  OrcStrView sv2    = orc_sv_from_str(buf2);
  TEST_ASSERT_TRUE(orc_sv_ends_with(sv2, "x"));
  TEST_ASSERT_TRUE(!orc_sv_ends_with(sv2, "yx"));
}

void test_orc_sv_contains_str(void)
{
  char       buf[] = "hello world";
  OrcStrView sv    = orc_sv_from_str(buf);
  TEST_ASSERT_TRUE(orc_sv_contains_str(sv, "hello"));
  TEST_ASSERT_TRUE(orc_sv_contains_str(sv, "world"));
  TEST_ASSERT_TRUE(orc_sv_contains_str(sv, "lo wo"));
  TEST_ASSERT_TRUE(orc_sv_contains_str(sv, "hello world"));
  TEST_ASSERT_TRUE(orc_sv_contains_str(sv, "h"));
  TEST_ASSERT_TRUE(orc_sv_contains_str(sv, "d"));
  TEST_ASSERT_TRUE(!orc_sv_contains_str(sv, "hello world!"));
  TEST_ASSERT_TRUE(!orc_sv_contains_str(sv, "xyz"));
  TEST_ASSERT_TRUE(!orc_sv_contains_str(sv, "Hello"));
  // Repeated first-byte partial matches (regression: infinite loop)
  char       buf2[] = "aaab";
  OrcStrView sv2    = orc_sv_from_str(buf2);
  TEST_ASSERT_TRUE(orc_sv_contains_str(sv2, "aab"));
  TEST_ASSERT_TRUE(!orc_sv_contains_str(sv2, "aac"));
  // Empty needle
  TEST_ASSERT_TRUE(!orc_sv_contains_str(sv, ""));
  // NULL needle
  TEST_ASSERT_TRUE(!orc_sv_contains_str(sv, NULL));
  // Empty view
  OrcStrView empty = orc_sv_from_str("");
  TEST_ASSERT_TRUE(!orc_sv_contains_str(empty, "a"));
  // NULL view
  OrcStrView null_sv = (OrcStrView) {0};
  TEST_ASSERT_TRUE(!orc_sv_contains_str(null_sv, "a"));
  // Single char view
  char       buf3[] = "x";
  OrcStrView sv3    = orc_sv_from_str(buf3);
  TEST_ASSERT_TRUE(orc_sv_contains_str(sv3, "x"));
  TEST_ASSERT_TRUE(!orc_sv_contains_str(sv3, "y"));
  TEST_ASSERT_TRUE(!orc_sv_contains_str(sv3, "xy"));
  // Needle same length as view, no match
  char       buf4[] = "abc";
  OrcStrView sv4    = orc_sv_from_str(buf4);
  TEST_ASSERT_TRUE(!orc_sv_contains_str(sv4, "abd"));
  TEST_ASSERT_TRUE(orc_sv_contains_str(sv4, "abc"));
}

void test_orc_sv_find(void)
{
  char       buf[] = "hello";
  OrcStrView sv    = orc_sv_from_str(buf);
  TEST_ASSERT_TRUE(orc_sv_find(sv, 'h') == buf);
  TEST_ASSERT_TRUE(orc_sv_find(sv, 'o') == buf + 4);
  TEST_ASSERT_TRUE(orc_sv_find(sv, 'l') == buf + 2);
  TEST_ASSERT_TRUE(orc_sv_find(sv, 'z') == NULL);
  // Empty view
  OrcStrView empty = orc_sv_from_str("");
  TEST_ASSERT_TRUE(orc_sv_find(empty, 'a') == NULL);
  // NULL view
  OrcStrView null_sv = (OrcStrView) {0};
  TEST_ASSERT_TRUE(orc_sv_find(null_sv, 'a') == NULL);
  // Single char view
  char       buf2[] = "x";
  OrcStrView sv2    = orc_sv_from_str(buf2);
  TEST_ASSERT_TRUE(orc_sv_find(sv2, 'x') == buf2);
  TEST_ASSERT_TRUE(orc_sv_find(sv2, 'y') == NULL);
}

void test_orc_sv_rfind(void)
{
  char       buf[] = "hello";
  OrcStrView sv    = orc_sv_from_str(buf);
  // Finds last occurrence
  TEST_ASSERT_TRUE(orc_sv_rfind(sv, 'l') == buf + 3);
  TEST_ASSERT_TRUE(orc_sv_rfind(sv, 'h') == buf);
  TEST_ASSERT_TRUE(orc_sv_rfind(sv, 'o') == buf + 4);
  TEST_ASSERT_TRUE(orc_sv_rfind(sv, 'z') == NULL);
  // All same characters
  char       buf2[] = "aaaa";
  OrcStrView sv2    = orc_sv_from_str(buf2);
  TEST_ASSERT_TRUE(orc_sv_rfind(sv2, 'a') == buf2 + 3);
  // Empty view (regression: out-of-bounds dereference)
  OrcStrView empty = orc_sv_from_str("");
  TEST_ASSERT_TRUE(orc_sv_rfind(empty, 'a') == NULL);
  // NULL view
  OrcStrView null_sv = (OrcStrView) {0};
  TEST_ASSERT_TRUE(orc_sv_rfind(null_sv, 'a') == NULL);
  // Single char view
  char       buf3[] = "x";
  OrcStrView sv3    = orc_sv_from_str(buf3);
  TEST_ASSERT_TRUE(orc_sv_rfind(sv3, 'x') == buf3);
  TEST_ASSERT_TRUE(orc_sv_rfind(sv3, 'y') == NULL);
  // Only first char matches
  char       buf4[] = "abc";
  OrcStrView sv4    = orc_sv_from_str(buf4);
  TEST_ASSERT_TRUE(orc_sv_rfind(sv4, 'a') == buf4);
  // Only last char matches
  TEST_ASSERT_TRUE(orc_sv_rfind(sv4, 'c') == buf4 + 2);
}

void test_orc_str_eq(void)
{
  // Equal strings
  char *a = NULL;
  char *b = NULL;
  TEST_ASSERT_TRUE(orc_str_push_str(a, "hello") == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(orc_str_push_str(b, "hello") == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(orc_str_eq(a, b));
  // Different strings, same length
  orc_str_clear(b);
  TEST_ASSERT_TRUE(orc_str_push_str(b, "world") == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(!orc_str_eq(a, b));
  // Different lengths
  orc_str_clear(b);
  TEST_ASSERT_TRUE(orc_str_push_str(b, "hi") == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(!orc_str_eq(a, b));
  // Both NULL
  TEST_ASSERT_TRUE(orc_str_eq(NULL, NULL));
  // One NULL
  TEST_ASSERT_TRUE(!orc_str_eq(a, NULL));
  TEST_ASSERT_TRUE(!orc_str_eq(NULL, a));
  // Both empty
  orc_str_clear(a);
  orc_str_clear(b);
  TEST_ASSERT_TRUE(orc_str_eq(a, b));
  orc_str_free(a);
  orc_str_free(b);
}

void test_orc_sv_contains_char(void)
{
  char       buf[] = "hello";
  OrcStrView sv    = orc_sv_from_str(buf);
  TEST_ASSERT_TRUE(orc_sv_contains_char(sv, 'h'));
  TEST_ASSERT_TRUE(orc_sv_contains_char(sv, 'o'));
  TEST_ASSERT_TRUE(!orc_sv_contains_char(sv, 'z'));
  // Empty and NULL views
  TEST_ASSERT_TRUE(!orc_sv_contains_char(orc_sv_from_str(""), 'a'));
  TEST_ASSERT_TRUE(!orc_sv_contains_char((OrcStrView) {0}, 'a'));
}

void test_orc_sv_strip_prefix(void)
{
  char       buf[] = "hello world";
  OrcStrView sv    = orc_sv_from_str(buf);
  // Successful strip
  TEST_ASSERT_TRUE(orc_sv_strip_prefix(&sv, "hello"));
  TEST_ASSERT_TRUE(orc_sv_len(sv) == 6);
  TEST_ASSERT_TRUE(memcmp(sv.start, " world", 6) == 0);
  // Strip again on remainder
  TEST_ASSERT_TRUE(orc_sv_strip_prefix(&sv, " "));
  TEST_ASSERT_TRUE(orc_sv_len(sv) == 5);
  TEST_ASSERT_TRUE(memcmp(sv.start, "world", 5) == 0);
  // Prefix not present
  TEST_ASSERT_TRUE(!orc_sv_strip_prefix(&sv, "xyz"));
  TEST_ASSERT_TRUE_MESSAGE(orc_sv_len(sv) == 5, "View unchanged on failed strip");
  // Prefix longer than view
  TEST_ASSERT_TRUE(!orc_sv_strip_prefix(&sv, "world!!!!"));
  // Strip entire view
  TEST_ASSERT_TRUE(orc_sv_strip_prefix(&sv, "world"));
  TEST_ASSERT_TRUE(orc_sv_is_empty(sv));
  // Empty view
  OrcStrView empty = orc_sv_from_str("");
  TEST_ASSERT_TRUE(!orc_sv_strip_prefix(&empty, "a"));
  // NULL view
  OrcStrView null_sv = (OrcStrView) {0};
  TEST_ASSERT_TRUE(!orc_sv_strip_prefix(&null_sv, "a"));
  // NULL prefix
  OrcStrView sv2 = orc_sv_from_str(buf);
  TEST_ASSERT_TRUE(!orc_sv_strip_prefix(&sv2, NULL));
  // NULL pointer to sv
  TEST_ASSERT_TRUE(!orc_sv_strip_prefix(NULL, "a"));
}

void test_orc_sv_strip_suffix(void)
{
  char       buf[] = "hello world";
  OrcStrView sv    = orc_sv_from_str(buf);
  // Successful strip
  TEST_ASSERT_TRUE(orc_sv_strip_suffix(&sv, "world"));
  TEST_ASSERT_TRUE(orc_sv_len(sv) == 6);
  TEST_ASSERT_TRUE(memcmp(sv.start, "hello ", 6) == 0);
  // Strip again on remainder
  TEST_ASSERT_TRUE(orc_sv_strip_suffix(&sv, " "));
  TEST_ASSERT_TRUE(orc_sv_len(sv) == 5);
  TEST_ASSERT_TRUE(memcmp(sv.start, "hello", 5) == 0);
  // Suffix not present
  TEST_ASSERT_TRUE(!orc_sv_strip_suffix(&sv, "xyz"));
  TEST_ASSERT_TRUE_MESSAGE(orc_sv_len(sv) == 5, "View unchanged on failed strip");
  // Suffix longer than view
  TEST_ASSERT_TRUE(!orc_sv_strip_suffix(&sv, "!!!!hello"));
  // Strip entire view
  TEST_ASSERT_TRUE(orc_sv_strip_suffix(&sv, "hello"));
  TEST_ASSERT_TRUE(orc_sv_is_empty(sv));
  // Empty view
  OrcStrView empty = orc_sv_from_str("");
  TEST_ASSERT_TRUE(!orc_sv_strip_suffix(&empty, "a"));
  // NULL view
  OrcStrView null_sv = (OrcStrView) {0};
  TEST_ASSERT_TRUE(!orc_sv_strip_suffix(&null_sv, "a"));
  // NULL suffix
  OrcStrView sv2 = orc_sv_from_str(buf);
  TEST_ASSERT_TRUE(!orc_sv_strip_suffix(&sv2, NULL));
  // NULL pointer to sv
  TEST_ASSERT_TRUE(!orc_sv_strip_suffix(NULL, "a"));
}

void test_orc_sv_slice(void)
{
  char       buf[] = "hello world";
  OrcStrView sv    = orc_sv_from_str(buf);
  // Slice from middle
  OrcStrView mid = orc_sv_slice(sv, 2, 7);
  TEST_ASSERT_TRUE(orc_sv_len(mid) == 5);
  TEST_ASSERT_TRUE(memcmp(mid.start, "llo w", 5) == 0);
  // Slice from start
  OrcStrView head = orc_sv_slice(sv, 0, 5);
  TEST_ASSERT_TRUE(orc_sv_len(head) == 5);
  TEST_ASSERT_TRUE(memcmp(head.start, "hello", 5) == 0);
  // Slice to end
  OrcStrView tail = orc_sv_slice(sv, 6, 11);
  TEST_ASSERT_TRUE(orc_sv_len(tail) == 5);
  TEST_ASSERT_TRUE(memcmp(tail.start, "world", 5) == 0);
  // Full slice
  OrcStrView full = orc_sv_slice(sv, 0, 11);
  TEST_ASSERT_TRUE(orc_sv_len(full) == 11);
  TEST_ASSERT_TRUE(memcmp(full.start, "hello world", 11) == 0);
  // Empty slice (start == end)
  OrcStrView empty_slice = orc_sv_slice(sv, 3, 3);
  TEST_ASSERT_TRUE(orc_sv_is_empty(empty_slice));
  TEST_ASSERT_TRUE(empty_slice.start != NULL);
  // Single char slice
  OrcStrView one = orc_sv_slice(sv, 0, 1);
  TEST_ASSERT_TRUE(orc_sv_len(one) == 1);
  TEST_ASSERT_TRUE(*one.start == 'h');
  // Invalid: end > view length
  OrcStrView bad1 = orc_sv_slice(sv, 0, 100);
  TEST_ASSERT_TRUE(bad1.start == NULL);
  TEST_ASSERT_TRUE(bad1.end == NULL);
  // Invalid: start > end
  OrcStrView bad2 = orc_sv_slice(sv, 5, 2);
  TEST_ASSERT_TRUE(bad2.start == NULL);
  TEST_ASSERT_TRUE(bad2.end == NULL);
  // NULL view
  OrcStrView null_sv = (OrcStrView) {0};
  OrcStrView bad3    = orc_sv_slice(null_sv, 0, 1);
  TEST_ASSERT_TRUE(bad3.start == NULL);
}

void test_orc_sv_eq(void)
{
  char       buf1[] = "hello";
  char       buf2[] = "hello";
  OrcStrView a      = orc_sv_from_str(buf1);
  OrcStrView b      = orc_sv_from_str(buf2);
  // Equal views (different backing memory)
  TEST_ASSERT_TRUE(orc_sv_eq(a, b));
  // Same view
  TEST_ASSERT_TRUE(orc_sv_eq(a, a));
  // Different content, same length
  char       buf3[] = "world";
  OrcStrView c      = orc_sv_from_str(buf3);
  TEST_ASSERT_TRUE(!orc_sv_eq(a, c));
  // Different lengths
  char       buf4[] = "hi";
  OrcStrView d      = orc_sv_from_str(buf4);
  TEST_ASSERT_TRUE(!orc_sv_eq(a, d));
  // Both empty
  OrcStrView e1 = orc_sv_from_str("");
  OrcStrView e2 = orc_sv_from_str("");
  TEST_ASSERT_TRUE(orc_sv_eq(e1, e2));
  // Both NULL
  OrcStrView n1 = (OrcStrView) {0};
  OrcStrView n2 = (OrcStrView) {0};
  TEST_ASSERT_TRUE(orc_sv_eq(n1, n2));
  // One empty, one NULL (both have len 0)
  TEST_ASSERT_TRUE(orc_sv_eq(e1, n1));
  // Empty vs non-empty
  TEST_ASSERT_TRUE(!orc_sv_eq(e1, a));
  // Compare sub-slices
  OrcStrView sub    = orc_sv_slice(a, 0, 3);
  char       buf5[] = "hel";
  OrcStrView match  = orc_sv_from_str(buf5);
  TEST_ASSERT_TRUE(orc_sv_eq(sub, match));
}

void test_orc_sdk_deck_header_alignment(void)
{
  TEST_ASSERT_TRUE_MESSAGE(
    (sizeof(_OrcSdk_DeckHeader) % sizeof(_MaxAlignCompat)) == 0,
    "Deck header must align with the platform's maximum alignment to be compatible "
    "with arbitrary types inside the container. This doesn't guarantee alignment with "
    "SIMD types. The containers are not meant to be used with SIMD types.");
}

// Helper: build a binary deck of the given depth (port of Rust's binary_deck).
static size_t *_binary_deck(uint8_t depth)
{
  size_t *deck = NULL;
  size_t  n    = (size_t)1 << depth;
  for (size_t i = 0; i < n; ++i) {
    uint8_t tz = (i > 0) ? (uint8_t)__builtin_ctzll(i) : depth;
    if (tz > depth)
      tz = depth;
    TEST_ASSERT_TRUE(orc_sdk_deck_push(deck, i, tz) == ORC_ERROR_NONE);
  }
  return deck;
}

void test_deck_basic_push_and_length(void)
{
  size_t *deck = NULL;
  TEST_ASSERT_TRUE(orc_sdk_deck_len(deck) == 0);
  TEST_ASSERT_TRUE(orc_sdk_deck_max_depth(deck) == 0);
  TEST_ASSERT_TRUE(orc_sdk_deck_is_empty(deck));
  TEST_ASSERT_TRUE(orc_sdk_deck_push(deck, ((size_t) {7}), 1) == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(orc_sdk_deck_len(deck) == 1);
  TEST_ASSERT_TRUE(orc_sdk_deck_max_depth(deck) == 1);
  TEST_ASSERT_TRUE(!orc_sdk_deck_is_empty(deck));
  TEST_ASSERT_TRUE(_orc_sdk_deck_header(deck)->item_size == sizeof(size_t));
  TEST_ASSERT_TRUE(deck[0] == 7);
  TEST_ASSERT_TRUE(orc_sdk_deck_push(deck, ((size_t) {8}), 0) == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(orc_sdk_deck_push(deck, ((size_t) {9}), 0) == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(orc_sdk_deck_len(deck) == 3);
  TEST_ASSERT_TRUE(_orc_sdk_deck_header(deck)->item_size == sizeof(size_t));
  TEST_ASSERT_TRUE(deck[0] == 7);
  TEST_ASSERT_TRUE(deck[1] == 8);
  TEST_ASSERT_TRUE(deck[2] == 9);
  orc_sdk_deck_free(deck);
}

void test_deck_binary_deck(void)
{
  uint8_t const DEPTH = 5;
  size_t       *deck  = _binary_deck(DEPTH);
  TEST_ASSERT_TRUE(orc_sdk_deck_len(deck) == (size_t)(1 << DEPTH));
  TEST_ASSERT_TRUE(orc_sdk_deck_max_depth(deck) == DEPTH);
  TEST_ASSERT_TRUE(_orc_sdk_deck_header(deck)->item_size == sizeof(size_t));
  for (size_t i = 0; i < orc_sdk_deck_len(deck); ++i) {
    TEST_ASSERT_TRUE(deck[i] == i);
  }
  orc_sdk_deck_free(deck);
}

void test_deck_mark_structure(void)
{
  // Depth-3 binary deck: marks at positions 0,2,4,6 with depths 2,0,1,0.
  size_t             *deck = _binary_deck(3);
  _OrcSdk_DeckHeader *h    = _orc_sdk_deck_header(deck);
  TEST_ASSERT_TRUE(h->item_size == sizeof(size_t));
  TEST_ASSERT_TRUE(orc_sdk_arr_len(h->marks) == 4);
  TEST_ASSERT_TRUE(h->marks[0].depth == 2);
  TEST_ASSERT_TRUE(h->marks[1].depth == 0);
  TEST_ASSERT_TRUE(h->marks[2].depth == 1);
  TEST_ASSERT_TRUE(h->marks[3].depth == 0);
  TEST_ASSERT_TRUE(h->marks[0].pos == 0);
  TEST_ASSERT_TRUE(h->marks[1].pos == 2);
  TEST_ASSERT_TRUE(h->marks[2].pos == 4);
  TEST_ASSERT_TRUE(h->marks[3].pos == 6);
  orc_sdk_deck_free(deck);
}
void test_orc_sdk_deck_clear(void)
{
  size_t *deck = _binary_deck(3);
  TEST_ASSERT_TRUE(orc_sdk_deck_len(deck) == 8);
  TEST_ASSERT_TRUE(_orc_sdk_deck_header(deck)->item_size == sizeof(size_t));
  orc_sdk_deck_clear(deck);
  TEST_ASSERT_TRUE(orc_sdk_deck_len(deck) == 0);
  TEST_ASSERT_TRUE(orc_sdk_deck_max_depth(deck) == 0);
  TEST_ASSERT_TRUE(_orc_sdk_deck_header(deck)->item_size == sizeof(size_t));
  // Re-use after clear.
  TEST_ASSERT_TRUE(orc_sdk_deck_push(deck, ((size_t) {1}), 1) == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(orc_sdk_deck_push(deck, ((size_t) {2}), 0) == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(orc_sdk_deck_len(deck) == 2);
  TEST_ASSERT_TRUE(orc_sdk_deck_max_depth(deck) == 1);
  TEST_ASSERT_TRUE(_orc_sdk_deck_header(deck)->item_size == sizeof(size_t));
  TEST_ASSERT_TRUE(deck[0] == 1);
  TEST_ASSERT_TRUE(deck[1] == 2);
  orc_sdk_deck_free(deck);
}

void test_orc_sdk_deck_flatten(void)
{
  size_t *deck = _binary_deck(4);
  size_t  n    = orc_sdk_deck_len(deck);
  TEST_ASSERT_TRUE(n == 16);
  TEST_ASSERT_TRUE(_orc_sdk_deck_header(deck)->item_size == sizeof(size_t));
  orc_sdk_deck_flatten(deck);
  TEST_ASSERT_TRUE(orc_sdk_deck_max_depth(deck) == 1);
  TEST_ASSERT_TRUE(_orc_sdk_deck_header(deck)->item_size == sizeof(size_t));
  for (size_t i = 0; i < n; ++i) {
    TEST_ASSERT_TRUE(deck[i] == i);
  }
  _OrcSdk_DeckHeader *h = _orc_sdk_deck_header(deck);
  TEST_ASSERT_TRUE(orc_sdk_arr_len(h->marks) == 1);
  TEST_ASSERT_TRUE(h->marks[0].depth == 0);
  TEST_ASSERT_TRUE(h->marks[0].pos == 0);
  // Flatten is idempotent.
  orc_sdk_deck_flatten(deck);
  TEST_ASSERT_TRUE(orc_sdk_deck_max_depth(deck) == 1);
  TEST_ASSERT_TRUE(orc_sdk_arr_len(h->marks) == 1);
  orc_sdk_deck_free(deck);
  // Flatten a single-element deck pushed at depth 0: no marks.
  size_t *deck2 = NULL;
  TEST_ASSERT_TRUE(orc_sdk_deck_push(deck2, ((size_t) {5}), 0) == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(_orc_sdk_deck_header(deck2)->item_size == sizeof(size_t));
  orc_sdk_deck_flatten(deck2);
  TEST_ASSERT_TRUE(orc_sdk_deck_max_depth(deck2) == 0);
  TEST_ASSERT_TRUE(_orc_sdk_deck_header(deck2)->item_size == sizeof(size_t));
  TEST_ASSERT_TRUE(orc_sdk_arr_len(_orc_sdk_deck_header(deck2)->marks) == 0);
  orc_sdk_deck_free(deck2);
}

void test_orc_sdk_deck_reserve(void)
{
  size_t *deck = NULL;
  TEST_ASSERT_TRUE(orc_sdk_deck_reserve(deck, 32) == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(deck != NULL);
  TEST_ASSERT_TRUE(orc_sdk_deck_len(deck) == 0);
  TEST_ASSERT_TRUE(_orc_sdk_deck_header(deck)->item_size == sizeof(size_t));
  for (size_t i = 0; i < 32; ++i) {
    TEST_ASSERT_TRUE(orc_sdk_deck_push(deck, i, (i == 0) ? 1 : 0) == ORC_ERROR_NONE);
  }
  TEST_ASSERT_TRUE(orc_sdk_deck_len(deck) == 32);
  TEST_ASSERT_TRUE(_orc_sdk_deck_header(deck)->item_size == sizeof(size_t));
  for (size_t i = 0; i < 32; ++i) {
    TEST_ASSERT_TRUE(deck[i] == i);
  }
  orc_sdk_deck_free(deck);
}

void test_deck_depth_clamping(void)
{
  // Depth higher than the first mark's depth should be clamped.
  size_t *deck = NULL;
  TEST_ASSERT_TRUE(orc_sdk_deck_push(deck, ((size_t) {0}), 2) ==
                   ORC_ERROR_NONE);  // first mark: internal depth 1
  TEST_ASSERT_TRUE(orc_sdk_deck_push(deck, ((size_t) {1}), 0) == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(orc_sdk_deck_push(deck, ((size_t) {2}), 5) ==
                   ORC_ERROR_NONE);  // should be clamped
  TEST_ASSERT_TRUE(orc_sdk_deck_push(deck, ((size_t) {3}), 0) == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(orc_sdk_deck_len(deck) == 4);
  TEST_ASSERT_TRUE(orc_sdk_deck_max_depth(deck) == 2);
  _OrcSdk_DeckHeader *h = _orc_sdk_deck_header(deck);
  TEST_ASSERT_TRUE(h->item_size == sizeof(size_t));
  TEST_ASSERT_TRUE(orc_sdk_arr_len(h->marks) == 2);
  TEST_ASSERT_TRUE(h->marks[0].depth == 1);
  TEST_ASSERT_TRUE(h->marks[1].depth <= h->marks[0].depth);
  orc_sdk_deck_free(deck);
}

void test_deck_single_element(void)
{
  // Depth 0: bare leaf, no marks.
  size_t *deck = NULL;
  TEST_ASSERT_TRUE(orc_sdk_deck_push(deck, ((size_t) {42}), 0) == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(orc_sdk_deck_len(deck) == 1);
  TEST_ASSERT_TRUE(orc_sdk_deck_max_depth(deck) == 0);
  TEST_ASSERT_TRUE(_orc_sdk_deck_header(deck)->item_size == sizeof(size_t));
  TEST_ASSERT_TRUE(deck[0] == 42);
  TEST_ASSERT_TRUE(orc_sdk_arr_len(_orc_sdk_deck_header(deck)->marks) == 0);
  orc_sdk_deck_free(deck);
  // Depth 1.
  size_t *deck2 = NULL;
  TEST_ASSERT_TRUE(orc_sdk_deck_push(deck2, ((size_t) {7}), 1) == ORC_ERROR_NONE);
  TEST_ASSERT_TRUE(orc_sdk_deck_len(deck2) == 1);
  TEST_ASSERT_TRUE(orc_sdk_deck_max_depth(deck2) == 1);
  TEST_ASSERT_TRUE(_orc_sdk_deck_header(deck2)->item_size == sizeof(size_t));
  TEST_ASSERT_TRUE(deck2[0] == 7);
  orc_sdk_deck_free(deck2);
}

void test_orc_sdk_deck_free_null(void)
{
  size_t *deck = NULL;
  orc_sdk_deck_free(deck);
  TEST_ASSERT_TRUE(deck == NULL);
}

void test_deck_many_pushes(void)
{
  size_t *deck = NULL;
  size_t  n    = 1000;
  for (size_t i = 0; i < n; ++i) {
    TEST_ASSERT_TRUE(orc_sdk_deck_push(deck, i, (i == 0) ? 1 : 0) == ORC_ERROR_NONE);
  }
  TEST_ASSERT_TRUE(orc_sdk_deck_len(deck) == n);
  TEST_ASSERT_TRUE(orc_sdk_deck_max_depth(deck) == 1);
  TEST_ASSERT_TRUE(_orc_sdk_deck_header(deck)->item_size == sizeof(size_t));
  for (size_t i = 0; i < n; ++i) {
    TEST_ASSERT_TRUE(deck[i] == i);
  }
  orc_sdk_deck_free(deck);
}

void test_orc_sdk_deck_graft(void)
{
  // Graft a depth-3 binary deck: depth should increase by 1, items unchanged.
  size_t *deck = _binary_deck(3);
  TEST_ASSERT_TRUE(_orc_sdk_deck_header(deck)->item_size == sizeof(size_t));
  orc_sdk_deck_graft(deck);
  TEST_ASSERT_TRUE(orc_sdk_deck_max_depth(deck) == 4);
  TEST_ASSERT_TRUE(_orc_sdk_deck_header(deck)->item_size == sizeof(size_t));
  for (size_t i = 0; i < 8; ++i) {
    TEST_ASSERT_TRUE(deck[i] == i);
  }
  // After graft, every original item should have its own depth-0 mark,
  // plus the original marks with depth incremented by 1.
  // Original marks: depths [2,0,1,0] at positions [0,2,4,6].
  // After graft, we expect 8 marks total (one per item position), with:
  //   pos 0: depth 3, pos 1: depth 0, pos 2: depth 1, pos 3: depth 0,
  //   pos 4: depth 2, pos 5: depth 0, pos 6: depth 1, pos 7: depth 0.
  _OrcSdk_DeckHeader *h = _orc_sdk_deck_header(deck);
  TEST_ASSERT_TRUE(orc_sdk_arr_len(h->marks) == 8);
  uint8_t const expected_depths[] = {3, 0, 1, 0, 2, 0, 1, 0};
  for (size_t i = 0; i < 8; ++i) {
    TEST_ASSERT_TRUE(h->marks[i].depth == expected_depths[i]);
    TEST_ASSERT_TRUE(h->marks[i].pos == i);
  }
  orc_sdk_deck_free(deck);
  // Graft then flatten roundtrip: items survive.
  size_t *deck2 = _binary_deck(2);
  orc_sdk_deck_graft(deck2);
  TEST_ASSERT_TRUE(_orc_sdk_deck_header(deck2)->item_size == sizeof(size_t));
  orc_sdk_deck_flatten(deck2);
  TEST_ASSERT_TRUE(orc_sdk_deck_max_depth(deck2) == 1);
  TEST_ASSERT_TRUE(_orc_sdk_deck_header(deck2)->item_size == sizeof(size_t));
  TEST_ASSERT_TRUE(orc_sdk_deck_len(deck2) == 4);
  for (size_t i = 0; i < 4; ++i) {
    TEST_ASSERT_TRUE(deck2[i] == i);
  }
  orc_sdk_deck_free(deck2);
  // Graft a flat (depth-1) deck: each item gets wrapped.
  size_t *deck3 = NULL;
  for (size_t i = 0; i < 3; ++i) {
    TEST_ASSERT_TRUE(orc_sdk_deck_push(deck3, i, (i == 0) ? 1 : 0) == ORC_ERROR_NONE);
  }
  TEST_ASSERT_TRUE(orc_sdk_deck_max_depth(deck3) == 1);
  orc_sdk_deck_graft(deck3);
  TEST_ASSERT_TRUE(orc_sdk_deck_max_depth(deck3) == 2);
  TEST_ASSERT_TRUE(_orc_sdk_deck_header(deck3)->item_size == sizeof(size_t));
  TEST_ASSERT_TRUE(orc_sdk_deck_len(deck3) == 3);
  // Every item should have its own mark at depth 0 (wrapped individually),
  // plus the original depth-0 mark promoted to depth 1.
  _OrcSdk_DeckHeader *h3 = _orc_sdk_deck_header(deck3);
  TEST_ASSERT_TRUE(orc_sdk_arr_len(h3->marks) == 3);
  TEST_ASSERT_TRUE(h3->marks[0].depth == 1);
  TEST_ASSERT_TRUE(h3->marks[1].depth == 0);
  TEST_ASSERT_TRUE(h3->marks[2].depth == 0);
  for (size_t i = 0; i < 3; ++i) {
    TEST_ASSERT_TRUE(h3->marks[i].pos == i);
  }
  orc_sdk_deck_free(deck3);
  // Graft an empty deck: should be a no-op.
  size_t *deck4 = NULL;
  orc_sdk_deck_graft(deck4);
  TEST_ASSERT_TRUE(orc_sdk_deck_len(deck4) == 0);
}

void test_orc_sdk_deck_simplify(void)
{
  // A deck whose mark depths already use every level is unchanged.
  {
    size_t             *deck = _binary_deck(3);
    _OrcSdk_DeckHeader *h    = _orc_sdk_deck_header(deck);
    TEST_ASSERT_TRUE(h->item_size == sizeof(size_t));
    size_t const n_marks = orc_sdk_arr_len(h->marks);
    uint8_t      depths_before[4];
    for (size_t i = 0; i < n_marks; ++i)
      depths_before[i] = h->marks[i].depth;
    orc_sdk_deck_simplify(deck);
    for (size_t i = 0; i < n_marks; ++i) {
      TEST_ASSERT_TRUE(h->marks[i].depth == depths_before[i]);
    }
    orc_sdk_deck_free(deck);
  }
  // A deck with gaps in depth levels: only depths 0 and 4 present.
  // Should be remapped to 0 and 1.
  {
    size_t *deck = NULL;
    TEST_ASSERT_TRUE(orc_sdk_deck_push(deck, ((size_t) {0}), 5) ==
                     ORC_ERROR_NONE);  // mark depth = 4
    TEST_ASSERT_TRUE(orc_sdk_deck_push(deck, ((size_t) {1}), 0) == ORC_ERROR_NONE);
    TEST_ASSERT_TRUE(orc_sdk_deck_push(deck, ((size_t) {2}), 2) ==
                     ORC_ERROR_NONE);  // mark depth = 1
    TEST_ASSERT_TRUE(orc_sdk_deck_push(deck, ((size_t) {3}), 0) == ORC_ERROR_NONE);
    _OrcSdk_DeckHeader *h = _orc_sdk_deck_header(deck);
    TEST_ASSERT_TRUE(orc_sdk_arr_len(h->marks) == 2);
    // Before simplify: depths are 4 and 1 (clamped from external 5 and 2).
    // Wait — first mark has internal depth 4, second gets clamped to 4.
    // But external 2 -> internal 1, which is <= 4, so no clamping.
    TEST_ASSERT_TRUE(h->marks[0].depth == 4);
    TEST_ASSERT_TRUE(h->marks[1].depth == 1);
    orc_sdk_deck_simplify(deck);
    TEST_ASSERT_TRUE(orc_sdk_deck_max_depth(deck) == 2);
    TEST_ASSERT_TRUE(h->item_size == sizeof(size_t));
    TEST_ASSERT_TRUE(h->marks[0].depth == 1);
    TEST_ASSERT_TRUE(h->marks[1].depth == 0);
    // Items unchanged.
    for (size_t i = 0; i < 4; ++i) {
      TEST_ASSERT_TRUE(deck[i] == i);
    }
    orc_sdk_deck_free(deck);
  }
  // Simplify is idempotent.
  {
    size_t *deck = NULL;
    TEST_ASSERT_TRUE(orc_sdk_deck_push(deck, ((size_t) {0}), 5) == ORC_ERROR_NONE);
    TEST_ASSERT_TRUE(orc_sdk_deck_push(deck, ((size_t) {1}), 0) == ORC_ERROR_NONE);
    TEST_ASSERT_TRUE(orc_sdk_deck_push(deck, ((size_t) {2}), 2) == ORC_ERROR_NONE);
    TEST_ASSERT_TRUE(orc_sdk_deck_push(deck, ((size_t) {3}), 0) == ORC_ERROR_NONE);
    orc_sdk_deck_simplify(deck);
    _OrcSdk_DeckHeader *h  = _orc_sdk_deck_header(deck);
    uint8_t const       d0 = h->marks[0].depth;
    uint8_t const       d1 = h->marks[1].depth;
    orc_sdk_deck_simplify(deck);
    TEST_ASSERT_TRUE(h->marks[0].depth == d0);
    TEST_ASSERT_TRUE(h->marks[1].depth == d1);
    orc_sdk_deck_free(deck);
  }
  // Simplify after graft.
  {
    size_t *deck = _binary_deck(3);
    orc_sdk_deck_graft(deck);
    TEST_ASSERT_TRUE(orc_sdk_deck_max_depth(deck) == 4);
    // After graft, depths are contiguous (0,1,2,3), so simplify is a no-op.
    _OrcSdk_DeckHeader *h             = _orc_sdk_deck_header(deck);
    size_t const        n_marks       = orc_sdk_arr_len(h->marks);
    uint8_t            *depths_before = NULL;
    for (size_t i = 0; i < n_marks; ++i) {
      orc_sdk_arr_push(depths_before, h->marks[i].depth);
    }
    orc_sdk_deck_simplify(deck);
    TEST_ASSERT_TRUE(orc_sdk_deck_max_depth(deck) == 4);
    for (size_t i = 0; i < n_marks; ++i) {
      TEST_ASSERT_TRUE(h->marks[i].depth == depths_before[i]);
    }
    orc_sdk_arr_free(depths_before);
    orc_sdk_deck_free(deck);
  }
  // Simplify on empty/NULL deck is safe.
  {
    size_t *deck = NULL;
    orc_sdk_deck_simplify(deck);
    TEST_ASSERT_TRUE(orc_sdk_deck_len(deck) == 0);
  }
}

void _print_size_t(void *item, char *dst, size_t len)
{
  size_t const val = *(size_t *)item;
  snprintf(dst, len, "%zu", val);
}

void test_deck_printf(void)
{
  size_t *deck = _binary_deck(5);
  TEST_ASSERT_TRUE(_orc_sdk_deck_header(deck)->item_size == sizeof(size_t));
  char *output = orc_sdk_deck_to_str(deck, _print_size_t);
  TEST_ASSERT_TRUE(orc_sv_eq(orc_sv_trim(orc_sv_from_str(output)),
                             orc_sv_trim(orc_sv_from_str("  5 ---------------| 0\n"
                                                         "                   | 1\n"
                                                         "              1 ---| 2\n"
                                                         "                   | 3\n"
                                                         "           2 ------| 4\n"
                                                         "                   | 5\n"
                                                         "              1 ---| 6\n"
                                                         "                   | 7\n"
                                                         "        3 ---------| 8\n"
                                                         "                   | 9\n"
                                                         "              1 ---| 10\n"
                                                         "                   | 11\n"
                                                         "           2 ------| 12\n"
                                                         "                   | 13\n"
                                                         "              1 ---| 14\n"
                                                         "                   | 15\n"
                                                         "     4 ------------| 16\n"
                                                         "                   | 17\n"
                                                         "              1 ---| 18\n"
                                                         "                   | 19\n"
                                                         "           2 ------| 20\n"
                                                         "                   | 21\n"
                                                         "              1 ---| 22\n"
                                                         "                   | 23\n"
                                                         "        3 ---------| 24\n"
                                                         "                   | 25\n"
                                                         "              1 ---| 26\n"
                                                         "                   | 27\n"
                                                         "           2 ------| 28\n"
                                                         "                   | 29\n"
                                                         "              1 ---| 30\n"
                                                         "                   | 31\n"))));
  orc_str_free(output);
  // Depth-2 with empty lists.
  ORC_SDK_DECK_INIT(deck, size_t, ((1, 2, 3), (), (4, 5, 6, 7), (), (8, 9, 10, 11), ()));
  output = orc_sdk_deck_to_str(deck, _print_size_t);
  TEST_ASSERT_TRUE(orc_sv_eq(orc_sv_trim(orc_sv_from_str(output)),
                             orc_sv_trim(orc_sv_from_str("  2 ------| 1\n"
                                                         "          | 2\n"
                                                         "          | 3\n"
                                                         "     1 ---|\n"
                                                         "     1 ---| 4\n"
                                                         "          | 5\n"
                                                         "          | 6\n"
                                                         "          | 7\n"
                                                         "     1 ---|\n"
                                                         "     1 ---| 8\n"
                                                         "          | 9\n"
                                                         "          | 10\n"
                                                         "          | 11\n"
                                                         "     1 ---|\n"))));
  orc_str_free(output);
  // Depth-1: Flat list.
  ORC_SDK_DECK_INIT(deck, size_t, (10, 20, 30));
  output = orc_sdk_deck_to_str(deck, _print_size_t);
  TEST_ASSERT_TRUE(orc_sv_eq(orc_sv_from_str(output),
                             orc_sv_from_str("  1 ---| 10\n"
                                             "       | 20\n"
                                             "       | 30\n")));
  orc_str_free(output);
  // List with single element.
  ORC_SDK_DECK_INIT(deck, size_t, (42));
  output = orc_sdk_deck_to_str(deck, _print_size_t);
  TEST_ASSERT_TRUE(orc_sv_eq(orc_sv_from_str(output), orc_sv_from_str("  1 ---| 42\n")));
  orc_str_free(output);
  // All empty depth-2.
  ORC_SDK_DECK_INIT(deck, size_t, ((), (), ()));
  output = orc_sdk_deck_to_str(deck, _print_size_t);
  TEST_ASSERT_TRUE(orc_sv_eq(orc_sv_from_str(output),
                             orc_sv_from_str("  2 ------|\n"
                                             "     1 ---|\n"
                                             "     1 ---|\n")));
  orc_str_free(output);
  // Empty deck.
  orc_sdk_deck_clear(deck);
  output = orc_sdk_deck_to_str(deck, _print_size_t);
  TEST_ASSERT_TRUE(output == NULL);
  // Depth-3 with nested empty.
  ORC_SDK_DECK_INIT(deck, size_t, (((1, 2), ()), (()), ((3))));
  output = orc_sdk_deck_to_str(deck, _print_size_t);
  TEST_ASSERT_TRUE(orc_sv_eq(orc_sv_from_str(output),
                             orc_sv_from_str("  3 ---------| 1\n"
                                             "             | 2\n"
                                             "        1 ---|\n"
                                             "     2 ------|\n"
                                             "     2 ------| 3\n")));
}

void test_deck_init(void)
{
  size_t             *deck = NULL;
  _OrcSdk_DeckHeader *h    = NULL;
  /* depth 1: flat list */
  ORC_SDK_DECK_INIT(deck, size_t, (10, 20, 30));
  TEST_ASSERT_TRUE(orc_sdk_deck_len(deck) == 3);
  TEST_ASSERT_TRUE(orc_sdk_deck_max_depth(deck) == 1);
  TEST_ASSERT_TRUE(_orc_sdk_deck_header(deck)->item_size == sizeof(size_t));
  TEST_ASSERT_TRUE(deck[0] == 10);
  TEST_ASSERT_TRUE(deck[1] == 20);
  TEST_ASSERT_TRUE(deck[2] == 30);
  h = _orc_sdk_deck_header(deck);
  TEST_ASSERT_TRUE(orc_sdk_arr_len(h->marks) == 1);
  TEST_ASSERT_TRUE(h->marks[0].depth == 0);
  TEST_ASSERT_TRUE(h->marks[0].pos == 0);
  /* depth 2: re-init clears existing data */
  ORC_SDK_DECK_INIT(deck, size_t, ((1, 2, 3), (4, 5, 6), (7, 8, 9)));
  TEST_ASSERT_TRUE(orc_sdk_deck_len(deck) == 9);
  TEST_ASSERT_TRUE(orc_sdk_deck_max_depth(deck) == 2);
  TEST_ASSERT_TRUE(_orc_sdk_deck_header(deck)->item_size == sizeof(size_t));
  for (size_t i = 0; i < 9; ++i)
    TEST_ASSERT_TRUE(deck[i] == i + 1);
  h = _orc_sdk_deck_header(deck);
  TEST_ASSERT_TRUE(orc_sdk_arr_len(h->marks) == 3);
  TEST_ASSERT_TRUE(h->marks[0].depth == 1);
  TEST_ASSERT_TRUE(h->marks[0].pos == 0);
  TEST_ASSERT_TRUE(h->marks[1].depth == 0);
  TEST_ASSERT_TRUE(h->marks[1].pos == 3);
  TEST_ASSERT_TRUE(h->marks[2].depth == 0);
  TEST_ASSERT_TRUE(h->marks[2].pos == 6);
  /* depth 3: ruler sequence depths 2,0,1,0 at positions 0,2,4,6 */
  ORC_SDK_DECK_INIT(deck, size_t, (((1, 2), (3, 4)), ((5, 6), (7, 8))));
  TEST_ASSERT_TRUE(orc_sdk_deck_len(deck) == 8);
  TEST_ASSERT_TRUE(orc_sdk_deck_max_depth(deck) == 3);
  TEST_ASSERT_TRUE(_orc_sdk_deck_header(deck)->item_size == sizeof(size_t));
  for (size_t i = 0; i < 8; ++i)
    TEST_ASSERT_TRUE(deck[i] == i + 1);
  h = _orc_sdk_deck_header(deck);
  TEST_ASSERT_TRUE(orc_sdk_arr_len(h->marks) == 4);
  TEST_ASSERT_TRUE(h->marks[0].depth == 2);
  TEST_ASSERT_TRUE(h->marks[0].pos == 0);
  TEST_ASSERT_TRUE(h->marks[1].depth == 0);
  TEST_ASSERT_TRUE(h->marks[1].pos == 2);
  TEST_ASSERT_TRUE(h->marks[2].depth == 1);
  TEST_ASSERT_TRUE(h->marks[2].pos == 4);
  TEST_ASSERT_TRUE(h->marks[3].depth == 0);
  TEST_ASSERT_TRUE(h->marks[3].pos == 6);
  orc_sdk_deck_free(deck);
}

// ========== OrcSdk_DeckView ==========

void test_dv_binary_deck(void)
{
  const uint8_t DEPTH = 5;
  size_t       *deck  = _binary_deck(DEPTH);
  TEST_ASSERT_TRUE(DEPTH == orc_sdk_deck_max_depth(deck));
  TEST_ASSERT_TRUE((size_t)(1 << DEPTH) == orc_sdk_deck_len(deck));
  TEST_ASSERT_TRUE(_orc_sdk_deck_header(deck)->item_size == sizeof(size_t));
  {  // Iterate from level 5.
    size_t          counter = 0;
    OrcSdk_DeckView v5      = orc_sdk_dv_from_deck(deck, 5);
    do {
      TEST_ASSERT_TRUE(5 == orc_sdk_dv_depth(&v5));
      TEST_ASSERT_TRUE(32 == orc_sdk_dv_len(&v5));
      OrcSdk_DeckView v4 = orc_sdk_dv_child(&v5);
      do {
        TEST_ASSERT_TRUE(4 == orc_sdk_dv_depth(&v4));
        TEST_ASSERT_TRUE(16 == orc_sdk_dv_len(&v4));
        OrcSdk_DeckView v3 = orc_sdk_dv_child(&v4);
        do {
          TEST_ASSERT_TRUE(3 == orc_sdk_dv_depth(&v3));
          TEST_ASSERT_TRUE(8 == orc_sdk_dv_len(&v3));
          OrcSdk_DeckView v2 = orc_sdk_dv_child(&v3);
          do {
            TEST_ASSERT_TRUE(2 == orc_sdk_dv_depth(&v2));
            TEST_ASSERT_TRUE(4 == orc_sdk_dv_len(&v2));
            OrcSdk_DeckView v1 = orc_sdk_dv_child(&v2);
            do {
              TEST_ASSERT_TRUE(1 == orc_sdk_dv_depth(&v1));
              TEST_ASSERT_TRUE(2 == orc_sdk_dv_len(&v1));
              size_t const *items = orc_sdk_dv_item_ptr(&v1);
              TEST_ASSERT_TRUE(items != NULL);
              size_t const *end = items + orc_sdk_dv_len(&v1);
              while (items != end) {
                TEST_ASSERT_TRUE(*(items++) == counter++);
              }
            } while (orc_sdk_dv_advance(&v1));
          } while (orc_sdk_dv_advance(&v2));
        } while (orc_sdk_dv_advance(&v3));
      } while (orc_sdk_dv_advance(&v4));
    } while (orc_sdk_dv_advance(&v5));
    TEST_ASSERT_TRUE(counter == 32);
  }
  {  // Iterate from level 4.
    OrcSdk_DeckView v4      = orc_sdk_dv_from_deck(deck, 4);
    size_t          counter = 0;
    do {
      TEST_ASSERT_TRUE(4 == orc_sdk_dv_depth(&v4));
      TEST_ASSERT_TRUE(16 == orc_sdk_dv_len(&v4));
      OrcSdk_DeckView v3 = orc_sdk_dv_child(&v4);
      do {
        TEST_ASSERT_TRUE(3 == orc_sdk_dv_depth(&v3));
        TEST_ASSERT_TRUE(8 == orc_sdk_dv_len(&v3));
        OrcSdk_DeckView v2 = orc_sdk_dv_child(&v3);
        do {
          TEST_ASSERT_TRUE(2 == orc_sdk_dv_depth(&v2));
          TEST_ASSERT_TRUE(4 == orc_sdk_dv_len(&v2));
          OrcSdk_DeckView v1 = orc_sdk_dv_child(&v2);
          do {
            TEST_ASSERT_TRUE(1 == orc_sdk_dv_depth(&v1));
            TEST_ASSERT_TRUE(2 == orc_sdk_dv_len(&v1));
            size_t const *items = orc_sdk_dv_item_ptr(&v1);
            TEST_ASSERT_TRUE(items != NULL);
            size_t const *end = items + orc_sdk_dv_len(&v1);
            while (items != end) {
              TEST_ASSERT_TRUE(*(items++) == counter++);
            }
          } while (orc_sdk_dv_advance(&v1));
        } while (orc_sdk_dv_advance(&v2));
      } while (orc_sdk_dv_advance(&v3));
    } while (orc_sdk_dv_advance(&v4));
    TEST_ASSERT_TRUE(counter == 32);
  }
  {  // Iterate from level 3.
    OrcSdk_DeckView v3      = orc_sdk_dv_from_deck(deck, 3);
    size_t          counter = 0;
    do {
      TEST_ASSERT_TRUE(3 == orc_sdk_dv_depth(&v3));
      TEST_ASSERT_TRUE(8 == orc_sdk_dv_len(&v3));
      OrcSdk_DeckView v2 = orc_sdk_dv_child(&v3);
      do {
        TEST_ASSERT_TRUE(2 == orc_sdk_dv_depth(&v2));
        TEST_ASSERT_TRUE(4 == orc_sdk_dv_len(&v2));
        OrcSdk_DeckView v1 = orc_sdk_dv_child(&v2);
        do {
          TEST_ASSERT_TRUE(1 == orc_sdk_dv_depth(&v1));
          TEST_ASSERT_TRUE(2 == orc_sdk_dv_len(&v1));
          OrcSdk_DeckView v0 = orc_sdk_dv_child(&v1);
          do {
            size_t const *item = orc_sdk_dv_item_ptr(&v0);
            TEST_ASSERT_TRUE(item != NULL);
            TEST_ASSERT_TRUE(*item == counter++);
          } while (orc_sdk_dv_advance(&v0));
        } while (orc_sdk_dv_advance(&v1));
      } while (orc_sdk_dv_advance(&v2));
    } while (orc_sdk_dv_advance(&v3));
    TEST_ASSERT_TRUE(counter == 32);
  }
}

// ========== OrcSdk_DeckWriter ==========

void test_dw_basic_depth2(void)
{
  // Build ((0,1,2),(3,4,5),(6,7,8)) via OrcSdk_DeckWriter, then verify via
  // OrcSdk_DeckView.
  uint32_t *deck    = NULL;
  uint32_t  counter = 0;
  {
    OrcSdk_DeckWriter w = orc_sdk_dw_from_deck(deck, 2);
    for (int g = 0; g < 3; ++g) {
      OrcSdk_DeckWriter c = orc_sdk_dw_child(&w);
      for (int i = 0; i < 3; ++i) {
        TEST_ASSERT_TRUE(orc_sdk_dw_push(&c, counter) == ORC_ERROR_NONE);
        counter++;
      }
      TEST_ASSERT_TRUE(orc_sdk_dw_len(&c) == 3);
    }
  }
  TEST_ASSERT_TRUE(orc_sdk_deck_len(deck) == 9);
  TEST_ASSERT_TRUE(orc_sdk_deck_max_depth(deck) == 2);
  TEST_ASSERT_TRUE(_orc_sdk_deck_header(deck)->item_size == sizeof(uint32_t));
  // Read back via OrcSdk_DeckView.
  counter            = 0;
  OrcSdk_DeckView v2 = orc_sdk_dv_from_deck(deck, 2);
  do {
    OrcSdk_DeckView v1 = orc_sdk_dv_child(&v2);
    do {
      uint32_t const *items = orc_sdk_dv_item_ptr(&v1);
      for (size_t i = 0; i < orc_sdk_dv_len(&v1); ++i) {
        TEST_ASSERT_TRUE(items[i] == counter++);
      }
    } while (orc_sdk_dv_advance(&v1));
  } while (orc_sdk_dv_advance(&v2));
  TEST_ASSERT_TRUE(counter == 9);
  orc_sdk_deck_free(deck);
}

void test_dw_depth3_nested(void)
{
  // Build 3x3x3 tree at depth 3, mirroring Rust t_deck_writer_basic.
  uint32_t *deck    = NULL;
  uint32_t  counter = 0;
  {
    OrcSdk_DeckWriter w3 = orc_sdk_dw_from_deck(deck, 3);
    TEST_ASSERT_TRUE(w3.depth == 3);
    for (int a = 0; a < 3; ++a) {
      OrcSdk_DeckWriter w2 = orc_sdk_dw_child(&w3);
      TEST_ASSERT_TRUE(w2.depth == 2);
      for (int b = 0; b < 3; ++b) {
        OrcSdk_DeckWriter w1 = orc_sdk_dw_child(&w2);
        TEST_ASSERT_TRUE(w1.depth == 1);
        for (int c = 0; c < 3; ++c) {
          TEST_ASSERT_TRUE(orc_sdk_dw_push(&w1, counter) == ORC_ERROR_NONE);
          counter++;
        }
        TEST_ASSERT_TRUE(orc_sdk_dw_len(&w1) == 3);
      }
    }
  }
  TEST_ASSERT_TRUE(orc_sdk_deck_len(deck) == 27);
  TEST_ASSERT_TRUE(orc_sdk_deck_max_depth(deck) == 3);
  TEST_ASSERT_TRUE(_orc_sdk_deck_header(deck)->item_size == sizeof(uint32_t));
  // Read back: iterate depth 3 → 2 → 1 → items.
  counter            = 0;
  OrcSdk_DeckView v3 = orc_sdk_dv_from_deck(deck, 3);
  do {
    OrcSdk_DeckView v2 = orc_sdk_dv_child(&v3);
    do {
      OrcSdk_DeckView v1 = orc_sdk_dv_child(&v2);
      do {
        uint32_t const *items = orc_sdk_dv_item_ptr(&v1);
        for (size_t i = 0; i < orc_sdk_dv_len(&v1); ++i) {
          TEST_ASSERT_TRUE(items[i] == counter++);
        }
      } while (orc_sdk_dv_advance(&v1));
    } while (orc_sdk_dv_advance(&v2));
  } while (orc_sdk_dv_advance(&v3));
  TEST_ASSERT_TRUE(counter == 27);
  orc_sdk_deck_free(deck);
}

void test_dw_unbalanced_tree(void)
{
  // ((1), (2,3,4), (5,6)) — groups of different sizes.
  uint32_t *deck = NULL;
  {
    OrcSdk_DeckWriter w = orc_sdk_dw_from_deck(deck, 2);
    {
      OrcSdk_DeckWriter c = orc_sdk_dw_child(&w);
      uint32_t          v = 1;
      TEST_ASSERT_TRUE(orc_sdk_dw_push(&c, v) == ORC_ERROR_NONE);
    }
    {
      OrcSdk_DeckWriter c = orc_sdk_dw_child(&w);
      uint32_t          v;
      v = 2;
      TEST_ASSERT_TRUE(orc_sdk_dw_push(&c, v) == ORC_ERROR_NONE);
      v = 3;
      TEST_ASSERT_TRUE(orc_sdk_dw_push(&c, v) == ORC_ERROR_NONE);
      v = 4;
      TEST_ASSERT_TRUE(orc_sdk_dw_push(&c, v) == ORC_ERROR_NONE);
    }
    {
      OrcSdk_DeckWriter c = orc_sdk_dw_child(&w);
      uint32_t          v;
      v = 5;
      TEST_ASSERT_TRUE(orc_sdk_dw_push(&c, v) == ORC_ERROR_NONE);
      v = 6;
      TEST_ASSERT_TRUE(orc_sdk_dw_push(&c, v) == ORC_ERROR_NONE);
    }
  }
  TEST_ASSERT_TRUE(orc_sdk_deck_len(deck) == 6);
  TEST_ASSERT_TRUE(_orc_sdk_deck_header(deck)->item_size == sizeof(uint32_t));
  // Verify structure.
  OrcSdk_DeckView outer = orc_sdk_dv_from_deck(deck, 2);
  OrcSdk_DeckView g1    = orc_sdk_dv_child(&outer);
  TEST_ASSERT_TRUE(orc_sdk_dv_len(&g1) == 1);
  TEST_ASSERT_TRUE(*(uint32_t *)orc_sdk_dv_item_ptr(&g1) == 1);
  orc_sdk_dv_advance(&g1);

  // Cannot reuse g1 after advance past end for depth>0,
  // so re-derive from outer after advance.
  orc_sdk_dv_advance(&outer);
  // But outer only has one top-level group, so we iterate children instead.
  // Let's just verify sequentially.
  uint32_t        expected[] = {1, 2, 3, 4, 5, 6};
  size_t          idx        = 0;
  OrcSdk_DeckView top        = orc_sdk_dv_from_deck(deck, 2);
  do {
    OrcSdk_DeckView inner = orc_sdk_dv_child(&top);
    do {
      uint32_t const *items = orc_sdk_dv_item_ptr(&inner);
      for (size_t i = 0; i < orc_sdk_dv_len(&inner); ++i) {
        TEST_ASSERT_TRUE(items[i] == expected[idx++]);
      }
    } while (orc_sdk_dv_advance(&inner));
  } while (orc_sdk_dv_advance(&top));
  TEST_ASSERT_TRUE(idx == 6);
  orc_sdk_deck_free(deck);
}

void test_dw_empty_groups(void)
{
  // Build ((), (1,2), (), (3), ()) via orc_sdk_dw_close for empty groups.
  uint32_t *deck = NULL;
  {
    OrcSdk_DeckWriter w = orc_sdk_dw_from_deck(deck, 2);
    {
      OrcSdk_DeckWriter c = orc_sdk_dw_child(&w);
      orc_sdk_dw_close(&c);  // empty group
    }
    {
      OrcSdk_DeckWriter c = orc_sdk_dw_child(&w);
      uint32_t          v;
      v = 1;
      TEST_ASSERT_TRUE(orc_sdk_dw_push(&c, v) == ORC_ERROR_NONE);
      v = 2;
      TEST_ASSERT_TRUE(orc_sdk_dw_push(&c, v) == ORC_ERROR_NONE);
    }
    {
      OrcSdk_DeckWriter c = orc_sdk_dw_child(&w);
      orc_sdk_dw_close(&c);  // empty group
    }
    {
      OrcSdk_DeckWriter c = orc_sdk_dw_child(&w);
      uint32_t          v = 3;
      TEST_ASSERT_TRUE(orc_sdk_dw_push(&c, v) == ORC_ERROR_NONE);
    }
    {
      OrcSdk_DeckWriter c = orc_sdk_dw_child(&w);
      orc_sdk_dw_close(&c);  // empty group
    }
  }
  TEST_ASSERT_TRUE(orc_sdk_deck_len(deck) == 3);
  TEST_ASSERT_TRUE(_orc_sdk_deck_header(deck)->item_size == sizeof(uint32_t));
  TEST_ASSERT_TRUE(deck[0] == 1);
  TEST_ASSERT_TRUE(deck[1] == 2);
  TEST_ASSERT_TRUE(deck[2] == 3);
  // Verify: 5 inner groups, sizes 0,2,0,1,0.
  size_t          group_sizes[] = {0, 2, 0, 1, 0};
  size_t          gi            = 0;
  OrcSdk_DeckView top           = orc_sdk_dv_from_deck(deck, 2);
  do {
    OrcSdk_DeckView inner = orc_sdk_dv_child(&top);
    do {
      TEST_ASSERT_TRUE(orc_sdk_dv_len(&inner) == group_sizes[gi++]);
    } while (orc_sdk_dv_advance(&inner));
  } while (orc_sdk_dv_advance(&top));
  TEST_ASSERT_TRUE(gi == 5);
  orc_sdk_deck_free(deck);
}

void test_dw_nested_empty(void)
{
  // Build (((), (1,2)), ((3,)), (())) at depth 3.
  uint32_t *deck = NULL;
  {
    OrcSdk_DeckWriter w3 = orc_sdk_dw_from_deck(deck, 3);
    {
      OrcSdk_DeckWriter w2 = orc_sdk_dw_child(&w3);
      {
        OrcSdk_DeckWriter w1 = orc_sdk_dw_child(&w2);
        orc_sdk_dw_close(&w1);  // empty inner
      }
      {
        OrcSdk_DeckWriter w1 = orc_sdk_dw_child(&w2);
        uint32_t          v;
        v = 1;
        TEST_ASSERT_TRUE(orc_sdk_dw_push(&w1, v) == ORC_ERROR_NONE);
        v = 2;
        TEST_ASSERT_TRUE(orc_sdk_dw_push(&w1, v) == ORC_ERROR_NONE);
      }
    }
    {
      OrcSdk_DeckWriter w2 = orc_sdk_dw_child(&w3);
      {
        OrcSdk_DeckWriter w1 = orc_sdk_dw_child(&w2);
        uint32_t          v  = 3;
        TEST_ASSERT_TRUE(orc_sdk_dw_push(&w1, v) == ORC_ERROR_NONE);
      }
    }
    {
      OrcSdk_DeckWriter w2 = orc_sdk_dw_child(&w3);
      {
        OrcSdk_DeckWriter w1 = orc_sdk_dw_child(&w2);
        orc_sdk_dw_close(&w1);  // empty inner
      }
    }
  }
  TEST_ASSERT_TRUE(orc_sdk_deck_len(deck) == 3);
  TEST_ASSERT_TRUE(orc_sdk_deck_max_depth(deck) == 3);
  TEST_ASSERT_TRUE(_orc_sdk_deck_header(deck)->item_size == sizeof(uint32_t));
  // Verify structure by iterating depth 3 → 2 → 1.
  // Expected: (((), (1,2)), ((3,)), (()))
  OrcSdk_DeckView v3 = orc_sdk_dv_from_deck(deck, 3);
  // Only one top-level group.
  OrcSdk_DeckView mid = orc_sdk_dv_child(&v3);
  // First mid group: ((), (1,2))
  {
    OrcSdk_DeckView inner = orc_sdk_dv_child(&mid);
    TEST_ASSERT_TRUE(orc_sdk_dv_len(&inner) == 0);  // empty
    TEST_ASSERT_TRUE(orc_sdk_dv_advance(&inner));
    TEST_ASSERT_TRUE(orc_sdk_dv_len(&inner) == 2);
    uint32_t const *items = orc_sdk_dv_item_ptr(&inner);
    TEST_ASSERT_TRUE(items[0] == 1);
    TEST_ASSERT_TRUE(items[1] == 2);
    TEST_ASSERT_TRUE(!orc_sdk_dv_advance(&inner));
  }
  TEST_ASSERT_TRUE(orc_sdk_dv_advance(&mid));
  // Second mid group: ((3,))
  {
    OrcSdk_DeckView inner = orc_sdk_dv_child(&mid);
    TEST_ASSERT_TRUE(orc_sdk_dv_len(&inner) == 1);
    TEST_ASSERT_TRUE(*(uint32_t *)orc_sdk_dv_item_ptr(&inner) == 3);
    TEST_ASSERT_TRUE(!orc_sdk_dv_advance(&inner));
  }
  TEST_ASSERT_TRUE(orc_sdk_dv_advance(&mid));
  // Third mid group: (())
  {
    OrcSdk_DeckView inner = orc_sdk_dv_child(&mid);
    TEST_ASSERT_TRUE(orc_sdk_dv_len(&inner) == 0);
    TEST_ASSERT_TRUE(!orc_sdk_dv_advance(&inner));
  }
  TEST_ASSERT_TRUE(!orc_sdk_dv_advance(&mid));
  orc_sdk_deck_free(deck);
}

void test_dw_single_element_deep(void)
{
  // One item wrapped at depth 5: (((((42)))))
  uint32_t *deck = NULL;
  {
    OrcSdk_DeckWriter w5 = orc_sdk_dw_from_deck(deck, 5);
    OrcSdk_DeckWriter w4 = orc_sdk_dw_child(&w5);
    OrcSdk_DeckWriter w3 = orc_sdk_dw_child(&w4);
    OrcSdk_DeckWriter w2 = orc_sdk_dw_child(&w3);
    OrcSdk_DeckWriter w1 = orc_sdk_dw_child(&w2);
    uint32_t          v  = 42;
    TEST_ASSERT_TRUE(orc_sdk_dw_push(&w1, v) == ORC_ERROR_NONE);
  }
  TEST_ASSERT_TRUE(orc_sdk_deck_len(deck) == 1);
  TEST_ASSERT_TRUE(orc_sdk_deck_max_depth(deck) == 5);
  TEST_ASSERT_TRUE(_orc_sdk_deck_header(deck)->item_size == sizeof(uint32_t));
  TEST_ASSERT_TRUE(deck[0] == 42);
  // Unwrap all the way down.
  OrcSdk_DeckView v5 = orc_sdk_dv_from_deck(deck, 5);
  OrcSdk_DeckView v4 = orc_sdk_dv_child(&v5);
  OrcSdk_DeckView v3 = orc_sdk_dv_child(&v4);
  OrcSdk_DeckView v2 = orc_sdk_dv_child(&v3);
  OrcSdk_DeckView v1 = orc_sdk_dv_child(&v2);
  TEST_ASSERT_TRUE(orc_sdk_dv_len(&v1) == 1);
  TEST_ASSERT_TRUE(*(uint32_t *)orc_sdk_dv_item_ptr(&v1) == 42);
  orc_sdk_deck_free(deck);
}

void test_orc_sdk_dw_len_tracking(void)
{
  // Verify orc_sdk_dw_len reflects items added at each scope level.
  uint32_t *deck = NULL;
  {
    OrcSdk_DeckWriter w3 = orc_sdk_dw_from_deck(deck, 3);
    TEST_ASSERT_TRUE(orc_sdk_dw_len(&w3) == 0);
    {
      OrcSdk_DeckWriter w2 = orc_sdk_dw_child(&w3);
      TEST_ASSERT_TRUE(orc_sdk_dw_len(&w2) == 0);
      {
        OrcSdk_DeckWriter w1 = orc_sdk_dw_child(&w2);
        TEST_ASSERT_TRUE(orc_sdk_dw_len(&w1) == 0);
        uint32_t v = 10;
        orc_sdk_dw_push(&w1, v);
        TEST_ASSERT_TRUE(orc_sdk_dw_len(&w1) == 1);
        v = 20;
        orc_sdk_dw_push(&w1, v);
        TEST_ASSERT_TRUE(orc_sdk_dw_len(&w1) == 2);
      }
      TEST_ASSERT_TRUE(orc_sdk_dw_len(&w2) == 2);
      {
        OrcSdk_DeckWriter w1 = orc_sdk_dw_child(&w2);
        uint32_t          v  = 30;
        orc_sdk_dw_push(&w1, v);
        TEST_ASSERT_TRUE(orc_sdk_dw_len(&w1) == 1);
      }
      TEST_ASSERT_TRUE(orc_sdk_dw_len(&w2) == 3);
    }
    TEST_ASSERT_TRUE(orc_sdk_dw_len(&w3) == 3);
    {
      OrcSdk_DeckWriter w2 = orc_sdk_dw_child(&w3);
      TEST_ASSERT_TRUE(orc_sdk_dw_len(&w2) == 0);
      {
        OrcSdk_DeckWriter w1 = orc_sdk_dw_child(&w2);
        uint32_t          v  = 40;
        orc_sdk_dw_push(&w1, v);
      }
      TEST_ASSERT_TRUE(orc_sdk_dw_len(&w2) == 1);
    }
    TEST_ASSERT_TRUE(orc_sdk_dw_len(&w3) == 4);
  }
  TEST_ASSERT_TRUE(orc_sdk_deck_len(deck) == 4);
  TEST_ASSERT_TRUE(_orc_sdk_deck_header(deck)->item_size == sizeof(uint32_t));
  orc_sdk_deck_free(deck);
}

void test_dw_append_to_existing(void)
{
  // Build ((1,2),(3,4)) manually, then append (5,6) via writer.
  uint32_t *deck = NULL;
  uint32_t  v;
  v = 1;
  TEST_ASSERT_TRUE(ORC_ERROR_NONE == orc_sdk_deck_push(deck, v, 2));
  v = 2;
  TEST_ASSERT_TRUE(ORC_ERROR_NONE == orc_sdk_deck_push(deck, v, 0));
  v = 3;
  TEST_ASSERT_TRUE(ORC_ERROR_NONE == orc_sdk_deck_push(deck, v, 1));
  v = 4;
  TEST_ASSERT_TRUE(ORC_ERROR_NONE == orc_sdk_deck_push(deck, v, 0));
  TEST_ASSERT_TRUE(orc_sdk_deck_len(deck) == 4);
  TEST_ASSERT_TRUE(_orc_sdk_deck_header(deck)->item_size == sizeof(uint32_t));
  // Append another depth-1 group.
  {
    OrcSdk_DeckWriter w = orc_sdk_dw_from_deck(deck, 1);
    v                   = 5;
    orc_sdk_dw_push(&w, v);
    v = 6;
    orc_sdk_dw_push(&w, v);
  }
  TEST_ASSERT_TRUE(orc_sdk_deck_len(deck) == 6);
  TEST_ASSERT_TRUE(_orc_sdk_deck_header(deck)->item_size == sizeof(uint32_t));
  // Verify: ((1,2),(3,4),(5,6))
  uint32_t        counter = 0;
  OrcSdk_DeckView top     = orc_sdk_dv_from_deck(deck, 2);
  do {
    OrcSdk_DeckView inner = orc_sdk_dv_child(&top);
    size_t          n     = 0;
    do {
      uint32_t const *items = orc_sdk_dv_item_ptr(&inner);
      for (size_t i = 0; i < orc_sdk_dv_len(&inner); ++i) {
        TEST_ASSERT_TRUE(items[i] == ++counter);
      }
      n++;
    } while (orc_sdk_dv_advance(&inner));
    TEST_ASSERT_TRUE(n * 2 <= 6);  // each group has 2 items
  } while (orc_sdk_dv_advance(&top));
  TEST_ASSERT_TRUE(counter == 6);
  orc_sdk_deck_free(deck);
}

void test_dw_flat_depth1(void)
{
  // Depth-1 writer: just a flat list.
  uint32_t *deck = NULL;
  {
    OrcSdk_DeckWriter w = orc_sdk_dw_from_deck(deck, 1);
    for (uint32_t i = 0; i < 5; ++i) {
      uint32_t v = i * 10;
      orc_sdk_dw_push(&w, v);
    }
    TEST_ASSERT_TRUE(orc_sdk_dw_len(&w) == 5);
  }
  TEST_ASSERT_TRUE(orc_sdk_deck_len(deck) == 5);
  TEST_ASSERT_TRUE(orc_sdk_deck_max_depth(deck) == 1);
  TEST_ASSERT_TRUE(_orc_sdk_deck_header(deck)->item_size == sizeof(uint32_t));
  for (uint32_t i = 0; i < 5; ++i) {
    TEST_ASSERT_TRUE(deck[i] == i * 10);
  }
  // Verify via view: one group with 5 items.
  OrcSdk_DeckView v1 = orc_sdk_dv_from_deck(deck, 1);
  TEST_ASSERT_TRUE(orc_sdk_dv_len(&v1) == 5);
  TEST_ASSERT_TRUE(!orc_sdk_dv_advance(&v1));
  orc_sdk_deck_free(deck);
}

void test_dw_orc_sdk_deck_item_ptr(void)
{
  // Verify orc_sdk_deck_item_ptr points to the right items.
  uint32_t *deck = NULL;
  {
    OrcSdk_DeckWriter w = orc_sdk_dw_from_deck(deck, 2);
    {
      OrcSdk_DeckWriter c = orc_sdk_dw_child(&w);
      uint32_t          v;
      v = 100;
      orc_sdk_dw_push(&c, v);
      v = 200;
      orc_sdk_dw_push(&c, v);
      v = 300;
      orc_sdk_dw_push(&c, v);
      uint32_t *items = orc_sdk_deck_item_ptr(&c);
      TEST_ASSERT_TRUE(items[0] == 100);
      TEST_ASSERT_TRUE(items[1] == 200);
      TEST_ASSERT_TRUE(items[2] == 300);
      TEST_ASSERT_TRUE(orc_sdk_dw_len(&c) == 3);
    }
    {
      OrcSdk_DeckWriter c = orc_sdk_dw_child(&w);
      uint32_t          v;
      v = 400;
      orc_sdk_dw_push(&c, v);
      v = 500;
      orc_sdk_dw_push(&c, v);
      uint32_t *items = orc_sdk_deck_item_ptr(&c);
      TEST_ASSERT_TRUE(items[0] == 400);
      TEST_ASSERT_TRUE(items[1] == 500);
      TEST_ASSERT_TRUE(orc_sdk_dw_len(&c) == 2);
    }
  }
  TEST_ASSERT_TRUE(orc_sdk_deck_len(deck) == 5);
  TEST_ASSERT_TRUE(_orc_sdk_deck_header(deck)->item_size == sizeof(uint32_t));
  orc_sdk_deck_free(deck);
}

void test_orc_sdk_dw_close_idempotent(void)
{
  // orc_sdk_dw_close on an already-written writer should be a no-op.
  // orc_sdk_dw_close on an already-closed writer should be a no-op.
  uint32_t *deck = NULL;
  {
    OrcSdk_DeckWriter w = orc_sdk_dw_from_deck(deck, 2);
    {
      // Writer that writes — close should be no-op.
      OrcSdk_DeckWriter c = orc_sdk_dw_child(&w);
      uint32_t          v = 1;
      orc_sdk_dw_push(&c, v);
      orc_sdk_dw_close(&c);  // has_next_depth already false
    }
    {
      // Writer that is closed twice.
      OrcSdk_DeckWriter c = orc_sdk_dw_child(&w);
      orc_sdk_dw_close(&c);  // creates empty group
      orc_sdk_dw_close(&c);  // should be no-op (zeroed)
    }
    {
      OrcSdk_DeckWriter c = orc_sdk_dw_child(&w);
      uint32_t          v = 2;
      orc_sdk_dw_push(&c, v);
    }
  }
  TEST_ASSERT_TRUE(orc_sdk_deck_len(deck) == 2);
  TEST_ASSERT_TRUE(_orc_sdk_deck_header(deck)->item_size == sizeof(uint32_t));
  // Verify: ((1), (), (2))
  size_t          group_sizes[] = {1, 0, 1};
  size_t          gi            = 0;
  OrcSdk_DeckView top           = orc_sdk_dv_from_deck(deck, 2);
  do {
    OrcSdk_DeckView inner = orc_sdk_dv_child(&top);
    do {
      TEST_ASSERT_TRUE(orc_sdk_dv_len(&inner) == group_sizes[gi++]);
    } while (orc_sdk_dv_advance(&inner));
  } while (orc_sdk_dv_advance(&top));
  TEST_ASSERT_TRUE(gi == 3);
  orc_sdk_deck_free(deck);
}

void test_dw_all_empty_depth3(void)
{
  // (((), ()), (())) — depth 3, no items at all.
  uint32_t *deck = NULL;
  {
    OrcSdk_DeckWriter w3 = orc_sdk_dw_from_deck(deck, 3);
    {
      OrcSdk_DeckWriter w2 = orc_sdk_dw_child(&w3);
      {
        OrcSdk_DeckWriter w1 = orc_sdk_dw_child(&w2);
        orc_sdk_dw_close(&w1);
      }
      {
        OrcSdk_DeckWriter w1 = orc_sdk_dw_child(&w2);
        orc_sdk_dw_close(&w1);
      }
    }
    {
      OrcSdk_DeckWriter w2 = orc_sdk_dw_child(&w3);
      {
        OrcSdk_DeckWriter w1 = orc_sdk_dw_child(&w2);
        orc_sdk_dw_close(&w1);
      }
    }
  }
  TEST_ASSERT_TRUE(orc_sdk_deck_len(deck) == 0);
  TEST_ASSERT_TRUE(orc_sdk_deck_max_depth(deck) == 3);
  TEST_ASSERT_TRUE(_orc_sdk_deck_header(deck)->item_size == sizeof(uint32_t));
  // Verify: one top group, two mid groups, inner groups all empty.
  OrcSdk_DeckView v3  = orc_sdk_dv_from_deck(deck, 3);
  OrcSdk_DeckView mid = orc_sdk_dv_child(&v3);
  // First mid: 2 empty children.
  {
    OrcSdk_DeckView inner = orc_sdk_dv_child(&mid);
    TEST_ASSERT_TRUE(orc_sdk_dv_len(&inner) == 0);
    TEST_ASSERT_TRUE(orc_sdk_dv_advance(&inner));
    TEST_ASSERT_TRUE(orc_sdk_dv_len(&inner) == 0);
    TEST_ASSERT_TRUE(!orc_sdk_dv_advance(&inner));
  }
  TEST_ASSERT_TRUE(orc_sdk_dv_advance(&mid));
  // Second mid: 1 empty child.
  {
    OrcSdk_DeckView inner = orc_sdk_dv_child(&mid);
    TEST_ASSERT_TRUE(orc_sdk_dv_len(&inner) == 0);
    TEST_ASSERT_TRUE(!orc_sdk_dv_advance(&inner));
  }
  TEST_ASSERT_TRUE(!orc_sdk_dv_advance(&mid));
  orc_sdk_deck_free(deck);
}

// ========== Dims (Units) ==========

void test_orc_sdk_dims_equal(void)
{
  OrcDims a = {1, 0, -2, 0, 0, 0, 0};
  OrcDims b = {1, 0, -2, 0, 0, 0, 0};
  OrcDims c = {1, 0, -1, 0, 0, 0, 0};
  TEST_ASSERT_TRUE(orc_sdk_dims_equal(a, b));
  TEST_ASSERT_TRUE(!orc_sdk_dims_equal(a, c));
  // Dimensionless
  OrcDims zero_a = {0, 0, 0, 0, 0, 0, 0};
  OrcDims zero_b = {0, 0, 0, 0, 0, 0, 0};
  TEST_ASSERT_TRUE(orc_sdk_dims_equal(zero_a, zero_b));
  // Differ only in last dimension
  OrcDims d = {0, 0, 0, 0, 0, 0, 1};
  TEST_ASSERT_TRUE(!orc_sdk_dims_equal(zero_a, d));
}

void test_orc_sdk_dims_multiply(void)
{
  // force * distance = energy
  OrcDims force  = {1, 1, -2, 0, 0, 0, 0};
  OrcDims length = {1, 0, 0, 0, 0, 0, 0};
  OrcDims out;
  orc_sdk_dims_multiply(force, length, out);
  OrcDims energy = {2, 1, -2, 0, 0, 0, 0};
  TEST_ASSERT_TRUE(orc_sdk_dims_equal(out, energy));
  // Multiply by dimensionless is identity
  OrcDims zero = {0, 0, 0, 0, 0, 0, 0};
  orc_sdk_dims_multiply(force, zero, out);
  TEST_ASSERT_TRUE(orc_sdk_dims_equal(out, force));
  // Negative exponents cancel
  OrcDims a = {-1, -1, 3, 0, 0, 0, 0};
  OrcDims b = {1, 1, -3, 0, 0, 0, 0};
  orc_sdk_dims_multiply(a, b, out);
  TEST_ASSERT_TRUE(orc_sdk_dims_equal(out, zero));
}

void test_orc_sdk_dims_divide(void)
{
  // velocity / time = acceleration
  OrcDims velocity = {1, 0, -1, 0, 0, 0, 0};
  OrcDims time     = {0, 0, 1, 0, 0, 0, 0};
  OrcDims out;
  orc_sdk_dims_divide(velocity, time, out);
  OrcDims accel = {1, 0, -2, 0, 0, 0, 0};
  TEST_ASSERT_TRUE(orc_sdk_dims_equal(out, accel));
  // Divide by self = dimensionless
  orc_sdk_dims_divide(velocity, velocity, out);
  OrcDims zero = {0, 0, 0, 0, 0, 0, 0};
  TEST_ASSERT_TRUE(orc_sdk_dims_equal(out, zero));
  // Divide dimensionless by something = negated exponents
  orc_sdk_dims_divide(zero, time, out);
  OrcDims inv_time = {0, 0, -1, 0, 0, 0, 0};
  TEST_ASSERT_TRUE(orc_sdk_dims_equal(out, inv_time));
}

void test_orc_sdk_dims_pow(void)
{
  OrcDims length = {1, 0, 0, 0, 0, 0, 0};
  OrcDims out;
  // length^2 = area
  orc_sdk_dims_pow(length, 2, out);
  OrcDims area = {2, 0, 0, 0, 0, 0, 0};
  TEST_ASSERT_TRUE(orc_sdk_dims_equal(out, area));
  // length^3 = volume
  orc_sdk_dims_pow(length, 3, out);
  OrcDims volume = {3, 0, 0, 0, 0, 0, 0};
  TEST_ASSERT_TRUE(orc_sdk_dims_equal(out, volume));
  // pow 0 = dimensionless
  OrcDims velocity = {1, 0, -1, 0, 0, 0, 0};
  orc_sdk_dims_pow(velocity, 0, out);
  OrcDims zero = {0, 0, 0, 0, 0, 0, 0};
  TEST_ASSERT_TRUE(orc_sdk_dims_equal(out, zero));
  // pow 1 = identity
  orc_sdk_dims_pow(velocity, 1, out);
  TEST_ASSERT_TRUE(orc_sdk_dims_equal(out, velocity));
  // Negative power
  orc_sdk_dims_pow(velocity, -1, out);
  OrcDims inv_vel = {-1, 0, 1, 0, 0, 0, 0};
  TEST_ASSERT_TRUE(orc_sdk_dims_equal(out, inv_vel));
  // pow -2 on multi-dim
  OrcDims force = {1, 1, -2, 0, 0, 0, 0};
  orc_sdk_dims_pow(force, -2, out);
  OrcDims expected = {-2, -2, 4, 0, 0, 0, 0};
  TEST_ASSERT_TRUE(orc_sdk_dims_equal(out, expected));
}

// ========== Plugin Functions ==========

void _print_double(void *item, char *dst, size_t len)
{
  double const val = *(double *)item;
  snprintf(dst, len, "%.2f", val);
}

void _print_uint32_t(void *item, char *dst, size_t len)
{
  uint32_t const val = *(uint32_t *)item;
  snprintf(dst, len, "%d", val);
}

// This simulates a function that will lives inside the plugin DLL. It takes a
// list<double>, a uin32_t and outputs the item from the list at that index.
void _plugin_function_list_element(OrcHandle const *list_handle,
                                   OrcHandle const *index_handle,
                                   OrcHandle       *item_handle)
{
  // Check the types of inpuuts.
  TEST_ASSERT_TRUE(list_handle->type_id == ORC_TYPE_F64);
  TEST_ASSERT_TRUE(index_handle->type_id == ORC_TYPE_U32);
  TEST_ASSERT_TRUE(item_handle->type_id == ORC_TYPE_F64);
  // Use the SDK provided combinatorics helper to stride over the input data.
  void *combinations =
    orc_sdk_comb_init((OrcHandle const *[]) {list_handle, index_handle},
                      (uint8_t const[]) {1, 0},
                      2,
                      (OrcHandle *[]) {item_handle},
                      (uint8_t const[]) {0},
                      1);
  while (combinations) {  // List processing iterations.
    // Get inputs for the current combination.
    OrcSdk_DeckView list_input  = orc_sdk_comb_get_input(combinations, 0),
                    index_input = orc_sdk_comb_get_input(combinations, 1);
    TEST_ASSERT_TRUE(list_input.depth == 1);
    TEST_ASSERT_TRUE(index_input.depth == 0);
    // Get output for the current combination.
    OrcSdk_DeckWriter *item_ouput = orc_sdk_comb_get_output(combinations, 0);
    TEST_ASSERT_TRUE(item_ouput->depth == 0);
    double *output_ptr = (double *)orc_sdk_dw_push_empty(item_ouput);
    {  // This scope simulates the actual doRun of the block.
      double        *list  = (double *)orc_sdk_dv_item_ptr(&list_input);
      uint32_t const index = *(uint32_t *)orc_sdk_dv_item_ptr(&index_input);
      TEST_ASSERT_TRUE_MESSAGE(index < orc_sdk_dv_len(&list_input),
                               "Index out of bounds");
      *output_ptr = list[index];  // Copy the output to the writer.
    }
    combinations = orc_sdk_comb_advance(combinations);
  }
  orc_sdk_comb_free(combinations);
}

// This simulates a function that takes two F64 scalars and outputs their sum.
void _plugin_function_add_f64(OrcHandle const *a_handle,
                              OrcHandle const *b_handle,
                              OrcHandle       *out_handle)
{
  TEST_ASSERT_TRUE(a_handle->type_id == ORC_TYPE_F64);
  TEST_ASSERT_TRUE(b_handle->type_id == ORC_TYPE_F64);
  TEST_ASSERT_TRUE(out_handle->type_id == ORC_TYPE_F64);
  void *combinations = orc_sdk_comb_init((OrcHandle const *[]) {a_handle, b_handle},
                                         (uint8_t const[]) {0, 0},
                                         2,
                                         (OrcHandle *[]) {out_handle},
                                         (uint8_t const[]) {0},
                                         1);
  while (combinations) {
    OrcSdk_DeckView a_input = orc_sdk_comb_get_input(combinations, 0),
                    b_input = orc_sdk_comb_get_input(combinations, 1);
    TEST_ASSERT_TRUE(a_input.depth == 0);
    TEST_ASSERT_TRUE(b_input.depth == 0);
    OrcSdk_DeckWriter *out = orc_sdk_comb_get_output(combinations, 0);
    TEST_ASSERT_TRUE(out->depth == 0);
    double *output_ptr = (double *)orc_sdk_dw_push_empty(out);
    {
      double const a = *(double *)orc_sdk_dv_item_ptr(&a_input);
      double const b = *(double *)orc_sdk_dv_item_ptr(&b_input);
      *output_ptr    = a + b;
    }
    combinations = orc_sdk_comb_advance(combinations);
  }
  orc_sdk_comb_free(combinations);
}

// This simulates a function that takes one F64 scalar and outputs its square and cube.
void _plugin_function_sq_cb(OrcHandle const *in_handle,
                            OrcHandle       *out_sq_handle,
                            OrcHandle       *out_cb_handle)
{
  TEST_ASSERT_TRUE(in_handle->type_id == ORC_TYPE_F64);
  TEST_ASSERT_TRUE(out_sq_handle->type_id == ORC_TYPE_F64);
  TEST_ASSERT_TRUE(out_cb_handle->type_id == ORC_TYPE_F64);
  void *combinations = orc_sdk_comb_init((OrcHandle const *[]) {in_handle},
                                         (uint8_t const[]) {0},
                                         1,
                                         (OrcHandle *[]) {out_sq_handle, out_cb_handle},
                                         (uint8_t const[]) {0, 0},
                                         2);
  while (combinations) {
    OrcSdk_DeckView in_input = orc_sdk_comb_get_input(combinations, 0);
    TEST_ASSERT_TRUE(in_input.depth == 0);
    OrcSdk_DeckWriter *out_sq = orc_sdk_comb_get_output(combinations, 0);
    OrcSdk_DeckWriter *out_cb = orc_sdk_comb_get_output(combinations, 1);
    TEST_ASSERT_TRUE(out_sq->depth == 0);
    TEST_ASSERT_TRUE(out_cb->depth == 0);
    double *sq_ptr = (double *)orc_sdk_dw_push_empty(out_sq);
    double *cb_ptr = (double *)orc_sdk_dw_push_empty(out_cb);
    {
      double const x = *(double *)orc_sdk_dv_item_ptr(&in_input);
      *sq_ptr        = x * x;
      *cb_ptr        = x * x * x;
    }
    combinations = orc_sdk_comb_advance(combinations);
  }
  orc_sdk_comb_free(combinations);
}

// This simulates a function that takes two F64 scalars and outputs their sum and product.
void _plugin_function_add_mul(OrcHandle const *a_handle,
                              OrcHandle const *b_handle,
                              OrcHandle       *out_sum_handle,
                              OrcHandle       *out_prod_handle)
{
  TEST_ASSERT_TRUE(a_handle->type_id == ORC_TYPE_F64);
  TEST_ASSERT_TRUE(b_handle->type_id == ORC_TYPE_F64);
  TEST_ASSERT_TRUE(out_sum_handle->type_id == ORC_TYPE_F64);
  TEST_ASSERT_TRUE(out_prod_handle->type_id == ORC_TYPE_F64);
  void *combinations =
    orc_sdk_comb_init((OrcHandle const *[]) {a_handle, b_handle},
                      (uint8_t const[]) {0, 0},
                      2,
                      (OrcHandle *[]) {out_sum_handle, out_prod_handle},
                      (uint8_t const[]) {0, 0},
                      2);
  while (combinations) {
    OrcSdk_DeckView a_input = orc_sdk_comb_get_input(combinations, 0),
                    b_input = orc_sdk_comb_get_input(combinations, 1);
    TEST_ASSERT_TRUE(a_input.depth == 0);
    TEST_ASSERT_TRUE(b_input.depth == 0);
    OrcSdk_DeckWriter *out_sum  = orc_sdk_comb_get_output(combinations, 0);
    OrcSdk_DeckWriter *out_prod = orc_sdk_comb_get_output(combinations, 1);
    TEST_ASSERT_TRUE(out_sum->depth == 0);
    TEST_ASSERT_TRUE(out_prod->depth == 0);
    double *sum_ptr  = (double *)orc_sdk_dw_push_empty(out_sum);
    double *prod_ptr = (double *)orc_sdk_dw_push_empty(out_prod);
    {
      double const a = *(double *)orc_sdk_dv_item_ptr(&a_input);
      double const b = *(double *)orc_sdk_dv_item_ptr(&b_input);
      *sum_ptr       = a + b;
      *prod_ptr      = a * b;
    }
    combinations = orc_sdk_comb_advance(combinations);
  }
  orc_sdk_comb_free(combinations);
}

void _plugin_function_first_add(OrcHandle const *a_handle,
                                OrcHandle const *b_handle,
                                OrcHandle       *out_handle)
{
  TEST_ASSERT_TRUE(a_handle->type_id == ORC_TYPE_F64);
  TEST_ASSERT_TRUE(b_handle->type_id == ORC_TYPE_F64);
  TEST_ASSERT_TRUE(out_handle->type_id == ORC_TYPE_F64);
  void *combinations = orc_sdk_comb_init((OrcHandle const *[]) {a_handle, b_handle},
                                         (uint8_t const[]) {1, 1},
                                         2,
                                         (OrcHandle *[]) {out_handle},
                                         (uint8_t const[]) {0},
                                         1);
  while (combinations) {
    OrcSdk_DeckView a_input = orc_sdk_comb_get_input(combinations, 0),
                    b_input = orc_sdk_comb_get_input(combinations, 1);
    TEST_ASSERT_TRUE(a_input.depth == 1);
    TEST_ASSERT_TRUE(b_input.depth == 1);
    OrcSdk_DeckWriter *out = orc_sdk_comb_get_output(combinations, 0);
    TEST_ASSERT_TRUE(out->depth == 0);
    double *output_ptr = (double *)orc_sdk_dw_push_empty(out);
    {
      TEST_ASSERT_TRUE_MESSAGE(
        orc_sdk_dv_len(&a_input) > 0 && orc_sdk_dv_len(&b_input) > 0,
        "Lists must be non-empty");
      double const a_first = *(double *)orc_sdk_dv_item_ptr(&a_input);
      double const b_first = *(double *)orc_sdk_dv_item_ptr(&b_input);
      *output_ptr          = a_first + b_first;
    }
    combinations = orc_sdk_comb_advance(combinations);
  }
  orc_sdk_comb_free(combinations);
}

void test_list_item_combinations(void)
{
  /*=== This test simulates the running of a list-element block. ===*/
  // Allocate decks - In a real scenario, the host program is allocating these,
  // by calling below functions, defined inside a plugin.
  OrcHandle lists = {0}, indices = {0}, out_items = {0};
  lists.handle     = 0;
  indices.handle   = 1;
  out_items.handle = 2;
  orc_sdk_handle_alloc(ORC_TYPE_F64, &lists);
  orc_sdk_handle_alloc(ORC_TYPE_U32, &indices);
  orc_sdk_handle_alloc(ORC_TYPE_F64, &out_items);
  TEST_ASSERT_TRUE_MESSAGE(
    lists.items != NULL && indices.items != NULL && out_items.items != NULL,
    "Unable to allocate decks");
  { /*Depth 2 lists, with one index.*/
    // Populate the inputs with data - in a real scenario this data is computed
    // by upstream functions. Here we pretend.
    ORC_SDK_DECK_INIT(lists.items,
                      double,
                      ((1.1, 2.23, 3.34, 3.14159),
                       (4.4, 5.5, 6.6, 6.28318),
                       (7.7, 8.8, 9.9, 10.1, 3.14159),
                       (11.1, 12.1, 13.1, 14.1, 15.1)));
    orc_sdk_oh_update(&lists);
    TEST_ASSERT_TRUE(orc_sdk_deck_len(lists.items) == 18);
    ORC_SDK_DECK_INIT(indices.items, uint32_t, 2);
    orc_sdk_oh_update(&indices);
    TEST_ASSERT_TRUE(orc_sdk_deck_len(indices.items) == 1);
    // Run the block - In a real scenario, this function is provided by a plugin DLL.
    _plugin_function_list_element(&lists, &indices, &out_items);
    // Check the outputs. The input had 4 lists, so the output should have 4 items.
    orc_sdk_oh_update(&out_items);
    size_t const count = orc_sdk_deck_len(out_items.items);
    TEST_ASSERT_TRUE(count == 4);
    TEST_ASSERT_TRUE(orc_sdk_deck_max_depth(out_items.items) == 1);
    // Output should contain the #2 item from every input list.
    double const  expected[] = {3.34, 6.6, 9.9, 13.1};
    double *const actual     = (double *)out_items.items;
    for (size_t i = 0; i < count; ++i) {
      TEST_ASSERT_TRUE(actual[i] == expected[i]);
    }
  }
  { /* Same depth 2 lists, with a list of indices. */
    ORC_SDK_DECK_INIT(indices.items, uint32_t, (0, 1, 2));
    orc_sdk_oh_update(&indices);
    orc_sdk_deck_clear(out_items.items);  // Clear the outputs.
    orc_sdk_oh_update(&out_items);
    // Run the block - In a real scenario, this function is provided by a plugin DLL.
    _plugin_function_list_element(&lists, &indices, &out_items);
    orc_sdk_oh_update(&out_items);
    size_t const count = orc_sdk_deck_len(out_items.items);
    TEST_ASSERT_TRUE(count == 4);
    TEST_ASSERT_TRUE(orc_sdk_deck_max_depth(out_items.items) == 1);
    double const  expected[] = {1.1, 5.5, 9.9, 13.1};
    double *const actual     = (double *)out_items.items;
    for (size_t i = 0; i < count; ++i) {
      TEST_ASSERT_TRUE(actual[i] == expected[i]);
    }
  }
  { /* Same depth 2 lists as before, with depth-2 indices. */
    ORC_SDK_DECK_INIT(indices.items, uint32_t, ((0, 1, 2), (1, 2, 3)));
    orc_sdk_oh_update(&indices);
    orc_sdk_deck_clear(out_items.items);  // Clear the outputs.
    orc_sdk_oh_update(&out_items);
    // Run the block - In a real scenario, this function is provided by a plugin DLL.
    _plugin_function_list_element(&lists, &indices, &out_items);
    orc_sdk_oh_update(&out_items);
    double *actual   = (double *)out_items.items;
    double *expected = NULL;
    ORC_SDK_DECK_INIT(expected, double, ((1.1, 5.5, 9.9, 13.1), (2.23, 6.6, 10.1, 14.1)));
    TEST_ASSERT_TRUE(orc_sdk_deck_max_depth(actual) == orc_sdk_deck_max_depth(expected));
    size_t n_marks = 0;
    {
      _OrcSdk_DeckHeader *h = _orc_sdk_deck_header(actual);
      n_marks               = orc_sdk_arr_len(h->marks);
      TEST_ASSERT_TRUE(orc_sdk_arr_len(_orc_sdk_deck_header(expected)->marks) == n_marks);
    }
    for (size_t i = 0; i < n_marks; ++i) {
      OrcMark const m1 = _orc_sdk_deck_header(expected)->marks[i];
      OrcMark const m2 = _orc_sdk_deck_header(actual)->marks[i];
      TEST_ASSERT_TRUE(m1.pos == m2.pos && m1.depth == m2.depth);
    }
    size_t const count = orc_sdk_deck_len(actual);
    TEST_ASSERT_TRUE(count == orc_sdk_deck_len(expected));
    for (size_t i = 0; i < count; ++i) {
      TEST_ASSERT_TRUE(actual[i] == expected[i]);
    }
    orc_sdk_deck_free(expected);
  }
  // Clean up decks - In a real scenario, the host program is cleaning up, by calling
  // below functions, which are defined inside a plugin.
  TEST_ASSERT_TRUE(ORC_ERROR_NONE == orc_sdk_handle_free(&lists));
  TEST_ASSERT_TRUE(ORC_ERROR_NONE == orc_sdk_handle_free(&indices));
  TEST_ASSERT_TRUE(ORC_ERROR_NONE == orc_sdk_handle_free(&out_items));
}

void test_add_f64_combinations(void)
{
  /*=== Tests two-input scalar addition: equal lengths, broadcast, and depth-2 inputs.
   * ===*/
  OrcHandle a = {0}, b = {0}, out = {0};
  a.handle   = 1;
  b.handle   = 2;
  out.handle = 3;
  orc_sdk_handle_alloc(ORC_TYPE_F64, &a);
  orc_sdk_handle_alloc(ORC_TYPE_F64, &b);
  orc_sdk_handle_alloc(ORC_TYPE_F64, &out);
  TEST_ASSERT_TRUE_MESSAGE(a.items != NULL && b.items != NULL && out.items != NULL,
                           "Unable to allocate decks");

  { /* Flat equal-length inputs: a and b each have 3 scalars (stack_depth=2). */
    ORC_SDK_DECK_INIT(a.items, double, (1.0, 2.0, 3.0));
    orc_sdk_oh_update(&a);
    ORC_SDK_DECK_INIT(b.items, double, (10.0, 20.0, 30.0));
    orc_sdk_oh_update(&b);
    _plugin_function_add_f64(&a, &b, &out);
    orc_sdk_oh_update(&out);
    size_t const count = orc_sdk_deck_len(out.items);
    TEST_ASSERT_TRUE(count == 3);
    TEST_ASSERT_TRUE(orc_sdk_deck_max_depth(out.items) == 1);
    double const  expected[] = {11.0, 22.0, 33.0};
    double *const actual     = (double *)out.items;
    for (size_t i = 0; i < count; ++i) {
      TEST_ASSERT_TRUE(actual[i] == expected[i]);
    }
  }
  { /* Flat inputs, broadcast-last: a has 4 scalars, b has 2 (stack_depth=2). */
    ORC_SDK_DECK_INIT(a.items, double, (1.0, 2.0, 3.0, 4.0));
    orc_sdk_oh_update(&a);
    ORC_SDK_DECK_INIT(b.items, double, (10.0, 20.0));
    orc_sdk_oh_update(&b);
    orc_sdk_deck_clear(out.items);
    orc_sdk_oh_update(&out);
    _plugin_function_add_f64(&a, &b, &out);
    orc_sdk_oh_update(&out);
    size_t const count = orc_sdk_deck_len(out.items);
    TEST_ASSERT_TRUE(count == 4);
    TEST_ASSERT_TRUE(orc_sdk_deck_max_depth(out.items) == 1);
    // b is exhausted at 20.0 and stays there for the remaining elements of a.
    double const  expected[] = {11.0, 22.0, 23.0, 24.0};
    double *const actual     = (double *)out.items;
    for (size_t i = 0; i < count; ++i) {
      TEST_ASSERT_TRUE(actual[i] == expected[i]);
    }
  }
  { /* Depth-2 inputs, equal groups: 2 inner groups of 2 items each (stack_depth=3). */
    ORC_SDK_DECK_INIT(a.items, double, ((1.0, 2.0), (3.0, 4.0)));
    orc_sdk_oh_update(&a);
    ORC_SDK_DECK_INIT(b.items, double, ((10.0, 20.0), (30.0, 40.0)));
    orc_sdk_oh_update(&b);
    orc_sdk_deck_clear(out.items);
    orc_sdk_oh_update(&out);
    _plugin_function_add_f64(&a, &b, &out);
    orc_sdk_oh_update(&out);
    double *actual   = (double *)out.items;
    double *expected = NULL;
    ORC_SDK_DECK_INIT(expected, double, ((11.0, 22.0), (33.0, 44.0)));
    TEST_ASSERT_TRUE(orc_sdk_deck_max_depth(actual) == orc_sdk_deck_max_depth(expected));
    size_t n_marks = 0;
    {
      _OrcSdk_DeckHeader *h = _orc_sdk_deck_header(actual);
      n_marks               = orc_sdk_arr_len(h->marks);
      TEST_ASSERT_TRUE(orc_sdk_arr_len(_orc_sdk_deck_header(expected)->marks) == n_marks);
    }
    for (size_t i = 0; i < n_marks; ++i) {
      OrcMark const m1 = _orc_sdk_deck_header(expected)->marks[i];
      OrcMark const m2 = _orc_sdk_deck_header(actual)->marks[i];
      TEST_ASSERT_TRUE(m1.pos == m2.pos && m1.depth == m2.depth);
    }
    size_t const count = orc_sdk_deck_len(actual);
    TEST_ASSERT_TRUE(count == orc_sdk_deck_len(expected));
    for (size_t i = 0; i < count; ++i) {
      TEST_ASSERT_TRUE(actual[i] == expected[i]);
    }
    orc_sdk_deck_free(expected);
  }
  { /* Depth-2 inputs, broadcast-last at inner-group level: a has 2 groups, b has 1
       (stack_depth=3). */
    ORC_SDK_DECK_INIT(a.items, double, ((1.0, 2.0), (3.0, 4.0)));
    orc_sdk_oh_update(&a);
    // b is a single depth-2 group (((10.0, 20.0))): b's one group broadcasts across a's
    // two.
    ORC_SDK_DECK_INIT(b.items, double, (((10.0, 20.0))));
    orc_sdk_oh_update(&b);
    orc_sdk_deck_clear(out.items);
    orc_sdk_oh_update(&out);

    _plugin_function_add_f64(&a, &b, &out);

    orc_sdk_oh_update(&out);
    double *actual   = (double *)out.items;
    double *expected = NULL;
    ORC_SDK_DECK_INIT(expected, double, (((11.0, 22.0), (13.0, 24.0))));
    TEST_ASSERT_TRUE(orc_sdk_deck_max_depth(actual) == orc_sdk_deck_max_depth(expected));
    size_t n_marks = 0;
    {
      _OrcSdk_DeckHeader *h = _orc_sdk_deck_header(actual);
      n_marks               = orc_sdk_arr_len(h->marks);
      TEST_ASSERT_TRUE(orc_sdk_arr_len(_orc_sdk_deck_header(expected)->marks) == n_marks);
    }
    for (size_t i = 0; i < n_marks; ++i) {
      OrcMark const m1 = _orc_sdk_deck_header(expected)->marks[i];
      OrcMark const m2 = _orc_sdk_deck_header(actual)->marks[i];
      TEST_ASSERT_TRUE(m1.pos == m2.pos && m1.depth == m2.depth);
    }
    size_t const count = orc_sdk_deck_len(actual);
    TEST_ASSERT_TRUE(count == orc_sdk_deck_len(expected));
    for (size_t i = 0; i < count; ++i) {
      TEST_ASSERT_TRUE(actual[i] == expected[i]);
    }
    orc_sdk_deck_free(expected);
  }
  TEST_ASSERT_TRUE(ORC_ERROR_NONE == orc_sdk_handle_free(&a));
  TEST_ASSERT_TRUE(ORC_ERROR_NONE == orc_sdk_handle_free(&b));
  TEST_ASSERT_TRUE(ORC_ERROR_NONE == orc_sdk_handle_free(&out));
}

// This simulates a function that takes depth=1 lists of F64 and outputs the length of
// each list as U64.
void _plugin_function_list_length(OrcHandle const *in_handle, OrcHandle *out_handle)
{
  TEST_ASSERT_TRUE(in_handle->type_id == ORC_TYPE_F64);
  TEST_ASSERT_TRUE(out_handle->type_id == ORC_TYPE_U64);
  void *combinations = orc_sdk_comb_init((OrcHandle const *[]) {in_handle},
                                         (uint8_t const[]) {1},
                                         1,
                                         (OrcHandle *[]) {out_handle},
                                         (uint8_t const[]) {0},
                                         1);
  while (combinations) {
    OrcSdk_DeckView    list_input = orc_sdk_comb_get_input(combinations, 0);
    OrcSdk_DeckWriter *out        = orc_sdk_comb_get_output(combinations, 0);
    TEST_ASSERT_TRUE(list_input.depth == 1);
    TEST_ASSERT_TRUE(out->depth == 0);
    uint64_t *output_ptr = (uint64_t *)orc_sdk_dw_push_empty(out);
    *output_ptr          = (uint64_t)orc_sdk_dv_len(&list_input);
    combinations         = orc_sdk_comb_advance(combinations);
  }
  orc_sdk_comb_free(combinations);
}

void test_list_length_combinations(void)
{
  /*=== Tests arg_depth=1 with U64 output: outputs the length of each input list, with
   * empty lists producing zeros. ===*/
  OrcHandle in  = {0};
  OrcHandle out = {0};
  in.handle     = 1;
  out.handle    = 2;
  orc_sdk_handle_alloc(ORC_TYPE_F64, &in);
  orc_sdk_handle_alloc(ORC_TYPE_U64, &out);
  TEST_ASSERT_TRUE_MESSAGE(in.items != NULL && out.items != NULL,
                           "Unable to allocate decks");
  { /* Depth-2 input: 5 lists, some empty (stack_depth=2). */
    ORC_SDK_DECK_INIT(in.items, double, ((1.0, 2.0, 3.0), (), (4.0), (), (5.0, 6.0)));
    orc_sdk_oh_update(&in);
    _plugin_function_list_length(&in, &out);
    orc_sdk_oh_update(&out);
    size_t const count = orc_sdk_deck_len(out.items);
    TEST_ASSERT_TRUE(count == 5);
    TEST_ASSERT_TRUE(orc_sdk_deck_max_depth(out.items) == 1);
    uint64_t const  expected[] = {3, 0, 1, 0, 2};
    uint64_t *const actual     = (uint64_t *)out.items;
    for (size_t i = 0; i < count; ++i) {
      TEST_ASSERT_TRUE(actual[i] == expected[i]);
    }
  }
  { /* Depth-3 input: two outer groups each containing lists, with empty lists inside
       (stack_depth=3). */
    ORC_SDK_DECK_INIT(in.items, double, (((1.0, 2.0), ()), ((3.0, 4.0, 5.0), (), (6.0))));
    orc_sdk_oh_update(&in);
    orc_sdk_deck_clear(out.items);
    orc_sdk_oh_update(&out);
    _plugin_function_list_length(&in, &out);
    orc_sdk_oh_update(&out);
    uint64_t *actual   = (uint64_t *)out.items;
    uint64_t *expected = NULL;
    ORC_SDK_DECK_INIT(expected, uint64_t, ((2, 0), (3, 0, 1)));
    TEST_ASSERT_TRUE(orc_sdk_deck_max_depth(actual) == orc_sdk_deck_max_depth(expected));
    size_t n_marks = 0;
    {
      _OrcSdk_DeckHeader *h = _orc_sdk_deck_header(actual);
      n_marks               = orc_sdk_arr_len(h->marks);
      TEST_ASSERT_TRUE(orc_sdk_arr_len(_orc_sdk_deck_header(expected)->marks) == n_marks);
    }
    for (size_t i = 0; i < n_marks; ++i) {
      OrcMark const m1 = _orc_sdk_deck_header(expected)->marks[i];
      OrcMark const m2 = _orc_sdk_deck_header(actual)->marks[i];
      TEST_ASSERT_TRUE(m1.pos == m2.pos && m1.depth == m2.depth);
    }
    size_t const count = orc_sdk_deck_len(actual);
    TEST_ASSERT_TRUE(count == orc_sdk_deck_len(expected));
    for (size_t i = 0; i < count; ++i) {
      TEST_ASSERT_TRUE(actual[i] == expected[i]);
    }
    orc_sdk_deck_free(expected);
  }
  TEST_ASSERT_TRUE(ORC_ERROR_NONE == orc_sdk_handle_free(&in));
  TEST_ASSERT_TRUE(ORC_ERROR_NONE == orc_sdk_handle_free(&out));
}

void test_two_output_combinations(void)
{
  /*=== Tests multiple-output Combinations: sq+cb (1 in, 2 out) and add+mul (2 in, 2 out).
   * ===*/
  OrcHandle in_a = {0}, in_b = {0}, out1 = {0}, out2 = {0};
  in_b.handle = 0;
  out1.handle = 1;
  out2.handle = 2;
  in_a.handle = 3;
  orc_sdk_handle_alloc(ORC_TYPE_F64, &in_a);
  orc_sdk_handle_alloc(ORC_TYPE_F64, &in_b);
  orc_sdk_handle_alloc(ORC_TYPE_F64, &out1);
  orc_sdk_handle_alloc(ORC_TYPE_F64, &out2);
  TEST_ASSERT_TRUE_MESSAGE(
    in_a.items != NULL && in_b.items != NULL && out1.items != NULL && out2.items != NULL,
    "Unable to allocate decks");

  { /* One input, two outputs: square and cube of 3 scalars (stack_depth=2). */
    ORC_SDK_DECK_INIT(in_a.items, double, (2.0, 3.0, 4.0));
    orc_sdk_oh_update(&in_a);

    _plugin_function_sq_cb(&in_a, &out1, &out2);

    orc_sdk_oh_update(&out1);
    orc_sdk_oh_update(&out2);
    TEST_ASSERT_TRUE(orc_sdk_deck_len(out1.items) == 3);
    TEST_ASSERT_TRUE(orc_sdk_deck_len(out2.items) == 3);
    TEST_ASSERT_TRUE(orc_sdk_deck_max_depth(out1.items) == 1);
    TEST_ASSERT_TRUE(orc_sdk_deck_max_depth(out2.items) == 1);
    double const  expected_sq[] = {4.0, 9.0, 16.0};
    double const  expected_cb[] = {8.0, 27.0, 64.0};
    double *const sq_actual     = (double *)out1.items;
    double *const cb_actual     = (double *)out2.items;
    for (size_t i = 0; i < 3; ++i) {
      TEST_ASSERT_TRUE(sq_actual[i] == expected_sq[i]);
      TEST_ASSERT_TRUE(cb_actual[i] == expected_cb[i]);
    }
  }
  { /* Two inputs, two outputs: sum and product of 3 scalars each (stack_depth=2). */
    ORC_SDK_DECK_INIT(in_a.items, double, (1.0, 2.0, 3.0));
    orc_sdk_oh_update(&in_a);
    ORC_SDK_DECK_INIT(in_b.items, double, (4.0, 5.0, 6.0));
    orc_sdk_oh_update(&in_b);
    orc_sdk_deck_clear(out1.items);
    orc_sdk_oh_update(&out1);
    orc_sdk_deck_clear(out2.items);
    orc_sdk_oh_update(&out2);

    _plugin_function_add_mul(&in_a, &in_b, &out1, &out2);

    orc_sdk_oh_update(&out1);
    orc_sdk_oh_update(&out2);
    TEST_ASSERT_TRUE(orc_sdk_deck_len(out1.items) == 3);
    TEST_ASSERT_TRUE(orc_sdk_deck_len(out2.items) == 3);
    TEST_ASSERT_TRUE(orc_sdk_deck_max_depth(out1.items) == 1);
    TEST_ASSERT_TRUE(orc_sdk_deck_max_depth(out2.items) == 1);
    double const  expected_sum[]  = {5.0, 7.0, 9.0};
    double const  expected_prod[] = {4.0, 10.0, 18.0};
    double *const sum_actual      = (double *)out1.items;
    double *const prod_actual     = (double *)out2.items;
    for (size_t i = 0; i < 3; ++i) {
      TEST_ASSERT_TRUE(sum_actual[i] == expected_sum[i]);
      TEST_ASSERT_TRUE(prod_actual[i] == expected_prod[i]);
    }
  }
  TEST_ASSERT_TRUE(ORC_ERROR_NONE == orc_sdk_handle_free(&in_a));
  TEST_ASSERT_TRUE(ORC_ERROR_NONE == orc_sdk_handle_free(&in_b));
  TEST_ASSERT_TRUE(ORC_ERROR_NONE == orc_sdk_handle_free(&out1));
  TEST_ASSERT_TRUE(ORC_ERROR_NONE == orc_sdk_handle_free(&out2));
}

void test_first_add_combinations(void)
{
  /*=== Tests arg_depth=1: plugin receives depth-1 list views and sums their first
   * elements. ===*/
  OrcHandle a = {0}, b = {0}, out = {0};
  b.handle   = 0;
  out.handle = 1;
  a.handle   = 2;
  orc_sdk_handle_alloc(ORC_TYPE_F64, &a);
  orc_sdk_handle_alloc(ORC_TYPE_F64, &b);
  orc_sdk_handle_alloc(ORC_TYPE_F64, &out);
  TEST_ASSERT_TRUE_MESSAGE(a.items != NULL && b.items != NULL && out.items != NULL,
                           "Unable to allocate decks");
  { /* Equal-length: a and b each have 3 depth=1 groups (stack_depth=2). */
    ORC_SDK_DECK_INIT(a.items, double, ((1.0, 99.0), (2.0, 99.0), (3.0, 99.0)));
    orc_sdk_oh_update(&a);
    ORC_SDK_DECK_INIT(b.items, double, ((10.0, 99.0), (20.0, 99.0), (30.0, 99.0)));
    orc_sdk_oh_update(&b);
    _plugin_function_first_add(&a, &b, &out);
    orc_sdk_oh_update(&out);
    size_t const count = orc_sdk_deck_len(out.items);
    TEST_ASSERT_TRUE(count == 3);
    TEST_ASSERT_TRUE(orc_sdk_deck_max_depth(out.items) == 1);
    double const  expected[] = {11.0, 22.0, 33.0};
    double *const actual     = (double *)out.items;
    for (size_t i = 0; i < count; ++i) {
      TEST_ASSERT_TRUE(actual[i] == expected[i]);
    }
  }
  { /* Broadcast-last at group level: a has 4 groups, b has 2 (stack_depth=2). */
    ORC_SDK_DECK_INIT(
      a.items, double, ((1.0, 99.0), (2.0, 99.0), (3.0, 99.0), (4.0, 99.0)));
    orc_sdk_oh_update(&a);
    ORC_SDK_DECK_INIT(b.items, double, ((10.0, 99.0), (20.0, 99.0)));
    orc_sdk_oh_update(&b);
    orc_sdk_deck_clear(out.items);
    orc_sdk_oh_update(&out);
    _plugin_function_first_add(&a, &b, &out);
    orc_sdk_oh_update(&out);
    size_t const count = orc_sdk_deck_len(out.items);
    TEST_ASSERT_TRUE(count == 4);
    TEST_ASSERT_TRUE(orc_sdk_deck_max_depth(out.items) == 1);
    // b is exhausted after its second group and stays at first(b[1])=20.0.
    double const  expected[] = {11.0, 22.0, 23.0, 24.0};
    double *const actual     = (double *)out.items;
    for (size_t i = 0; i < count; ++i) {
      TEST_ASSERT_TRUE(actual[i] == expected[i]);
    }
  }
  TEST_ASSERT_TRUE(ORC_ERROR_NONE == orc_sdk_handle_free(&a));
  TEST_ASSERT_TRUE(ORC_ERROR_NONE == orc_sdk_handle_free(&b));
  TEST_ASSERT_TRUE(ORC_ERROR_NONE == orc_sdk_handle_free(&out));
}

// ==================== Shuffling decks with a proxy ====================

static OrcHandle _make_flattened_proxy(void const *deck)
{
  _OrcSdk_DeckHeader *h     = _orc_sdk_deck_header(deck);
  OrcMark            *marks = NULL;
  OrcMark const       mark  = (OrcMark) {.depth = 0, .pos = 0};
  orc_sdk_arr_push(marks, mark);
  return (OrcHandle) {
    .handle        = 0,
    .items         = NULL,
    .n_items       = 0,
    .item_size     = h->item_size,
    .marks         = marks,
    .stride_offset = NULL,
    .n_marks       = 1,
    .strides       = NULL,
    .type_id       = ORC_TYPE_PROXY,
    .dims          = {0},
  };
}

static OrcHandle _make_grafted_proxy(void const *deck)
{
  _OrcSdk_DeckHeader *h         = _orc_sdk_deck_header(deck);
  OrcMark            *old_marks = h->marks;
  size_t const        n_marks   = orc_sdk_arr_len(old_marks);
  OrcMark            *marks     = NULL;
  uint64_t            prev      = 0;
  for (size_t i = 0; i < n_marks; ++i) {
    uint8_t const  new_depth = old_marks[i].depth + 1;
    uint64_t const current   = old_marks[i].pos;
    for (uint64_t j = prev; j < current; ++j) {
      OrcMark const m = {.depth = 0, .pos = j};
      orc_sdk_arr_push(marks, m);
    }
    {
      OrcMark const m = {.depth = new_depth, .pos = current};
      orc_sdk_arr_push(marks, m);
    }
    prev = current + 1;
  }
  for (uint64_t j = prev; j < (uint64_t)h->count; ++j) {
    OrcMark const m = {.depth = 0, .pos = j};
    orc_sdk_arr_push(marks, m);
  }
  return (OrcHandle) {
    .handle        = 0,
    .items         = NULL,
    .n_items       = 0,
    .item_size     = h->item_size,
    .marks         = marks,
    .stride_offset = NULL,
    .n_marks       = (uint64_t)orc_sdk_arr_len(marks),
    .strides       = NULL,
    .type_id       = ORC_TYPE_PROXY,
    .dims          = {0},
  };
}

static OrcHandle _make_simplified_proxy(void const *deck)
{
  _OrcSdk_DeckHeader *h       = _orc_sdk_deck_header(deck);
  size_t const        n_marks = orc_sdk_arr_len(h->marks);
  OrcMark            *marks   = NULL;
  if (n_marks == 0) {
    return (OrcHandle) {
      .type_id = ORC_TYPE_PROXY,
    };
  }
  uint8_t remap[256] = {0};
  {
    size_t const d_max = (size_t)h->marks[0].depth;
    for (size_t i = 0; i < n_marks; ++i) {
      remap[h->marks[i].depth] = 1;
    }
    uint8_t acc = 0;
    for (size_t r = 0; r <= d_max; ++r) {
      uint8_t const p = acc;
      acc += remap[r];
      remap[r] = p;
    }
  }
  for (size_t i = 0; i < n_marks; ++i) {
    OrcMark const m = {.depth = remap[h->marks[i].depth], .pos = h->marks[i].pos};
    orc_sdk_arr_push(marks, m);
  }
  return (OrcHandle) {
    .handle        = 0,
    .items         = NULL,
    .n_items       = 0,
    .item_size     = h->item_size,
    .marks         = marks,
    .stride_offset = NULL,
    .n_marks       = (uint64_t)orc_sdk_arr_len(marks),
    .strides       = NULL,
    .type_id       = ORC_TYPE_PROXY,
    .dims          = {0},
  };
}

static OrcHandle _make_shuffle_proxy(OrcItemProxy const *pdeck)
{
  OrcHandle handle;
  memset(&handle, 0, sizeof(handle));
  handle.items   = pdeck;
  handle.type_id = ORC_TYPE_PROXY;
  orc_sdk_oh_update(&handle);
  handle.type_id = ORC_TYPE_PROXY;
  return handle;
}

static void _assert_decks_match(void const  *actual,
                                void const  *expected,
                                size_t const item_size)
{
  size_t const na = orc_sdk_deck_len(actual);
  size_t const ne = orc_sdk_deck_len(expected);
  TEST_ASSERT_TRUE(na == ne);
  TEST_ASSERT_TRUE(memcmp(actual, expected, na * item_size) == 0);
  TEST_ASSERT_TRUE(orc_sdk_deck_max_depth(actual) == orc_sdk_deck_max_depth(expected));
  _OrcSdk_DeckHeader *ha  = _orc_sdk_deck_header(actual);
  _OrcSdk_DeckHeader *he  = _orc_sdk_deck_header(expected);
  size_t const        nma = orc_sdk_arr_len(ha->marks);
  size_t const        nme = orc_sdk_arr_len(he->marks);
  TEST_ASSERT_TRUE(nma == nme);
  for (size_t i = 0; i < nma; ++i) {
    TEST_ASSERT_TRUE(ha->marks[i].pos == he->marks[i].pos);
    TEST_ASSERT_TRUE(ha->marks[i].depth == he->marks[i].depth);
  }
}

void test_deck_from_proxy_copy_items(void)
{
  /*=== COPY_ITEMS: copies items from input, structure (marks) from proxy. ===*/
  { /* Flatten a depth-2 deck. */
    OrcHandle in = {0}, out = {0};
    in.handle  = 1;
    out.handle = 2;
    orc_sdk_handle_alloc(ORC_TYPE_F64, &in);
    ORC_SDK_DECK_INIT(in.items, double, ((1.0, 2.0), (3.0, 4.0, 5.0)));
    orc_sdk_oh_update(&in);
    OrcHandle proxy = _make_flattened_proxy(in.items);
    proxy.handle    = 3;
    orc_sdk_deck_from_proxy(&in, 1, ORC_DECK_PROXY_COPY_ITEMS, &proxy, &out);
    TEST_ASSERT_TRUE(out.type_id == ORC_TYPE_F64);
    TEST_ASSERT_TRUE(orc_sdk_deck_len(out.items) == 5);
    TEST_ASSERT_TRUE(orc_sdk_deck_max_depth(out.items) == 1);
    double *actual = (double *)out.items;
    TEST_ASSERT_TRUE(actual[0] == 1.0 && actual[1] == 2.0 && actual[2] == 3.0);
    TEST_ASSERT_TRUE(actual[3] == 4.0 && actual[4] == 5.0);
    orc_sdk_arr_free(proxy.marks);
    TEST_ASSERT_TRUE(ORC_ERROR_NONE == orc_sdk_handle_free(&out));
    TEST_ASSERT_TRUE(ORC_ERROR_NONE == orc_sdk_handle_free(&in));
  }
  { /* Flatten a depth-3 deck. */
    OrcHandle in = {0}, out = {0};
    in.handle  = 1;
    out.handle = 2;
    orc_sdk_handle_alloc(ORC_TYPE_F64, &in);
    ORC_SDK_DECK_INIT(in.items, double, (((1.0, 2.0), (3.0)), ((4.0, 5.0))));
    orc_sdk_oh_update(&in);
    OrcHandle proxy = _make_flattened_proxy(in.items);
    proxy.handle    = 3;
    orc_sdk_deck_from_proxy(&in, 1, ORC_DECK_PROXY_COPY_ITEMS, &proxy, &out);
    TEST_ASSERT_TRUE(orc_sdk_deck_len(out.items) == 5);
    TEST_ASSERT_TRUE(orc_sdk_deck_max_depth(out.items) == 1);
    double *actual = (double *)out.items;
    TEST_ASSERT_TRUE(actual[0] == 1.0 && actual[4] == 5.0);
    orc_sdk_arr_free(proxy.marks);
    TEST_ASSERT_TRUE(ORC_ERROR_NONE == orc_sdk_handle_free(&out));
    TEST_ASSERT_TRUE(ORC_ERROR_NONE == orc_sdk_handle_free(&in));
  }
  { /* Graft a flat deck: (1, 2, 3) → ((1), (2), (3)). */
    OrcHandle in = {0}, out = {0};
    in.handle  = 1;
    out.handle = 2;
    orc_sdk_handle_alloc(ORC_TYPE_F64, &in);
    ORC_SDK_DECK_INIT(in.items, double, (1.0, 2.0, 3.0));
    orc_sdk_oh_update(&in);
    OrcHandle proxy = _make_grafted_proxy(in.items);
    proxy.handle    = 3;
    orc_sdk_deck_from_proxy(&in, 1, ORC_DECK_PROXY_COPY_ITEMS, &proxy, &out);
    double *expected = NULL;
    ORC_SDK_DECK_INIT(expected, double, ((1.0), (2.0), (3.0)));
    _assert_decks_match(out.items, expected, sizeof(double));
    TEST_ASSERT_TRUE(out.type_id == ORC_TYPE_F64);
    orc_sdk_deck_free(expected);
    orc_sdk_arr_free(proxy.marks);
    TEST_ASSERT_TRUE(ORC_ERROR_NONE == orc_sdk_handle_free(&out));
    TEST_ASSERT_TRUE(ORC_ERROR_NONE == orc_sdk_handle_free(&in));
  }
  { /* Graft a depth-2 deck: ((1, 2), (3)) → (((1, 2)), ((3))). */
    OrcHandle in = {0}, out = {0};
    in.handle  = 1;
    out.handle = 2;
    orc_sdk_handle_alloc(ORC_TYPE_F64, &in);
    ORC_SDK_DECK_INIT(in.items, double, ((1.0, 2.0), (3.0)));
    orc_sdk_oh_update(&in);
    OrcHandle proxy = _make_grafted_proxy(in.items);
    proxy.handle    = 3;
    orc_sdk_deck_from_proxy(&in, 1, ORC_DECK_PROXY_COPY_ITEMS, &proxy, &out);
    double *expected = NULL;
    ORC_SDK_DECK_INIT(expected, double, (((1.0), (2.0)), ((3.0))));
    _assert_decks_match(out.items, expected, sizeof(double));
    orc_sdk_deck_free(expected);
    orc_sdk_arr_free(proxy.marks);
    TEST_ASSERT_TRUE(ORC_ERROR_NONE == orc_sdk_handle_free(&out));
    TEST_ASSERT_TRUE(ORC_ERROR_NONE == orc_sdk_handle_free(&in));
  }
  { /* Simplify: remove gaps in depth levels. */
    OrcHandle in = {0}, out = {0};
    in.handle  = 1;
    out.handle = 2;
    orc_sdk_handle_alloc(ORC_TYPE_F64, &in);
    ORC_SDK_DECK_INIT(in.items, double, ((1.0, 2.0), (3.0, 4.0)));
    orc_sdk_oh_update(&in);
    /* Graft to create a gap in depth levels, then use simplify proxy. */
    orc_sdk_deck_graft((void *)in.items);
    orc_sdk_oh_update(&in);
    OrcHandle proxy = _make_simplified_proxy(in.items);
    proxy.handle    = 3;
    orc_sdk_deck_from_proxy(&in, 1, ORC_DECK_PROXY_COPY_ITEMS, &proxy, &out);
    /* Simplify should match orc_sdk_deck_simplify on an equivalent deck. */
    double *expected = NULL;
    ORC_SDK_DECK_INIT(expected, double, ((1.0, 2.0), (3.0, 4.0)));
    orc_sdk_deck_graft(expected);
    orc_sdk_deck_simplify(expected);
    _assert_decks_match(out.items, expected, sizeof(double));
    orc_sdk_deck_free(expected);
    orc_sdk_arr_free(proxy.marks);
    TEST_ASSERT_TRUE(ORC_ERROR_NONE == orc_sdk_handle_free(&out));
    TEST_ASSERT_TRUE(ORC_ERROR_NONE == orc_sdk_handle_free(&in));
  }
}

void test_deck_from_proxy_shuffle(void)
{
  /*=== SHUFFLE: copies items one-at-a-time using proxy ItemProxy references. ===*/
  { /* Flat reverse: (1, 2, 3) → (3, 2, 1). */
    OrcHandle in = {0}, out = {0};
    in.handle  = 1;
    out.handle = 2;
    orc_sdk_handle_alloc(ORC_TYPE_F64, &in);
    ORC_SDK_DECK_INIT(in.items, double, (1.0, 2.0, 3.0));
    orc_sdk_oh_update(&in);
    OrcItemProxy *pdeck = NULL;
    TEST_ASSERT_TRUE(orc_sdk_deck_push(pdeck,
                                       ((OrcItemProxy) {.tree = 0, .item = 2}),
                                       1) == ORC_ERROR_NONE);
    TEST_ASSERT_TRUE(orc_sdk_deck_push(pdeck,
                                       ((OrcItemProxy) {.tree = 0, .item = 1}),
                                       0) == ORC_ERROR_NONE);
    TEST_ASSERT_TRUE(orc_sdk_deck_push(pdeck,
                                       ((OrcItemProxy) {.tree = 0, .item = 0}),
                                       0) == ORC_ERROR_NONE);
    OrcHandle proxy = _make_shuffle_proxy(pdeck);
    orc_sdk_deck_from_proxy(&in, 1, ORC_DECK_PROXY_SHUFFLE, &proxy, &out);
    double *expected = NULL;
    ORC_SDK_DECK_INIT(expected, double, (3.0, 2.0, 1.0));
    _assert_decks_match(out.items, expected, sizeof(double));
    TEST_ASSERT_TRUE(out.type_id == ORC_TYPE_F64);
    orc_sdk_deck_free(expected);
    orc_sdk_deck_free(pdeck);
    TEST_ASSERT_TRUE(ORC_ERROR_NONE == orc_sdk_handle_free(&out));
    TEST_ASSERT_TRUE(ORC_ERROR_NONE == orc_sdk_handle_free(&in));
  }
  { /* Nested reverse: ((1, 2), (3, 4, 5)) → ((2, 1), (5, 4, 3)). */
    OrcHandle in = {0}, out = {0};
    in.handle  = 1;
    out.handle = 2;
    orc_sdk_handle_alloc(ORC_TYPE_F64, &in);
    ORC_SDK_DECK_INIT(in.items, double, ((1.0, 2.0), (3.0, 4.0, 5.0)));
    orc_sdk_oh_update(&in);
    OrcItemProxy *pdeck = NULL;
    /* First sublist reversed: flat indices 1, 0. */
    TEST_ASSERT_TRUE(orc_sdk_deck_push(pdeck,
                                       ((OrcItemProxy) {.tree = 0, .item = 1}),
                                       2) == ORC_ERROR_NONE);
    TEST_ASSERT_TRUE(orc_sdk_deck_push(pdeck,
                                       ((OrcItemProxy) {.tree = 0, .item = 0}),
                                       0) == ORC_ERROR_NONE);
    /* Second sublist reversed: flat indices 4, 3, 2. */
    TEST_ASSERT_TRUE(orc_sdk_deck_push(pdeck,
                                       ((OrcItemProxy) {.tree = 0, .item = 4}),
                                       1) == ORC_ERROR_NONE);
    TEST_ASSERT_TRUE(orc_sdk_deck_push(pdeck,
                                       ((OrcItemProxy) {.tree = 0, .item = 3}),
                                       0) == ORC_ERROR_NONE);
    TEST_ASSERT_TRUE(orc_sdk_deck_push(pdeck,
                                       ((OrcItemProxy) {.tree = 0, .item = 2}),
                                       0) == ORC_ERROR_NONE);
    OrcHandle proxy = _make_shuffle_proxy(pdeck);
    orc_sdk_deck_from_proxy(&in, 1, ORC_DECK_PROXY_SHUFFLE, &proxy, &out);
    double *expected = NULL;
    ORC_SDK_DECK_INIT(expected, double, ((2.0, 1.0), (5.0, 4.0, 3.0)));
    _assert_decks_match(out.items, expected, sizeof(double));
    orc_sdk_deck_free(expected);
    orc_sdk_deck_free(pdeck);
    TEST_ASSERT_TRUE(ORC_ERROR_NONE == orc_sdk_handle_free(&out));
    TEST_ASSERT_TRUE(ORC_ERROR_NONE == orc_sdk_handle_free(&in));
  }
  { /* Generic list_item: pick item at index 1 from each sublist.
       ((1, 2, 3), (4, 5)) → (2, 5). */
    OrcHandle in = {0}, out = {0};
    in.handle  = 1;
    out.handle = 2;
    orc_sdk_handle_alloc(ORC_TYPE_F64, &in);
    ORC_SDK_DECK_INIT(in.items, double, ((1.0, 2.0, 3.0), (4.0, 5.0)));
    orc_sdk_oh_update(&in);
    OrcItemProxy *pdeck = NULL;
    TEST_ASSERT_TRUE(orc_sdk_deck_push(pdeck,
                                       ((OrcItemProxy) {.tree = 0, .item = 1}),
                                       1) == ORC_ERROR_NONE);
    TEST_ASSERT_TRUE(orc_sdk_deck_push(pdeck,
                                       ((OrcItemProxy) {.tree = 0, .item = 4}),
                                       0) == ORC_ERROR_NONE);
    OrcHandle proxy = _make_shuffle_proxy(pdeck);
    orc_sdk_deck_from_proxy(&in, 1, ORC_DECK_PROXY_SHUFFLE, &proxy, &out);
    TEST_ASSERT_TRUE(out.type_id == ORC_TYPE_F64);
    TEST_ASSERT_TRUE(orc_sdk_deck_len(out.items) == 2);
    double *actual = (double *)out.items;
    TEST_ASSERT_TRUE(actual[0] == 2.0 && actual[1] == 5.0);
    orc_sdk_deck_free(pdeck);
    TEST_ASSERT_TRUE(ORC_ERROR_NONE == orc_sdk_handle_free(&out));
    TEST_ASSERT_TRUE(ORC_ERROR_NONE == orc_sdk_handle_free(&in));
  }
  { /* Multi-input shuffle: interleave from two decks.
       A=(1, 2), B=(10, 20) → (1, 10, 2, 20). */
    OrcHandle a = {0}, b = {0}, out = {0};
    a.handle   = 1;
    b.handle   = 2;
    out.handle = 3;
    orc_sdk_handle_alloc(ORC_TYPE_F64, &a);
    orc_sdk_handle_alloc(ORC_TYPE_F64, &b);
    ORC_SDK_DECK_INIT(a.items, double, (1.0, 2.0));
    orc_sdk_oh_update(&a);
    ORC_SDK_DECK_INIT(b.items, double, (10.0, 20.0));
    orc_sdk_oh_update(&b);
    OrcItemProxy *pdeck = NULL;
    TEST_ASSERT_TRUE(orc_sdk_deck_push(pdeck,
                                       ((OrcItemProxy) {.tree = 0, .item = 0}),
                                       1) == ORC_ERROR_NONE);
    TEST_ASSERT_TRUE(orc_sdk_deck_push(pdeck,
                                       ((OrcItemProxy) {.tree = 1, .item = 0}),
                                       0) == ORC_ERROR_NONE);
    TEST_ASSERT_TRUE(orc_sdk_deck_push(pdeck,
                                       ((OrcItemProxy) {.tree = 0, .item = 1}),
                                       0) == ORC_ERROR_NONE);
    TEST_ASSERT_TRUE(orc_sdk_deck_push(pdeck,
                                       ((OrcItemProxy) {.tree = 1, .item = 1}),
                                       0) == ORC_ERROR_NONE);
    OrcHandle proxy     = _make_shuffle_proxy(pdeck);
    proxy.handle        = 4;
    OrcHandle inputs[2] = {a, b};
    orc_sdk_deck_from_proxy(inputs, 2, ORC_DECK_PROXY_SHUFFLE, &proxy, &out);
    double *expected = NULL;
    ORC_SDK_DECK_INIT(expected, double, (1.0, 10.0, 2.0, 20.0));
    _assert_decks_match(out.items, expected, sizeof(double));
    orc_sdk_deck_free(expected);
    orc_sdk_deck_free(pdeck);
    TEST_ASSERT_TRUE(ORC_ERROR_NONE == orc_sdk_handle_free(&out));
    TEST_ASSERT_TRUE(ORC_ERROR_NONE == orc_sdk_handle_free(&b));
    TEST_ASSERT_TRUE(ORC_ERROR_NONE == orc_sdk_handle_free(&a));
  }
}

void test_deck_from_proxy_type_agnostic(void)
{
  /*=== Verifies orc_deck_from_proxy preserves type across u32, i32, i16. ===*/
  { /* u32 flatten. */
    OrcHandle in = {0}, out = {0};
    in.handle  = 1;
    out.handle = 2;
    orc_sdk_handle_alloc(ORC_TYPE_U32, &in);
    ORC_SDK_DECK_INIT(in.items, uint32_t, ((10, 20), (30)));
    orc_sdk_oh_update(&in);
    OrcHandle proxy = _make_flattened_proxy(in.items);
    orc_sdk_deck_from_proxy(&in, 1, ORC_DECK_PROXY_COPY_ITEMS, &proxy, &out);
    TEST_ASSERT_TRUE(out.type_id == ORC_TYPE_U32);
    TEST_ASSERT_TRUE(orc_sdk_deck_len(out.items) == 3);
    TEST_ASSERT_TRUE(orc_sdk_deck_max_depth(out.items) == 1);
    uint32_t *actual = (uint32_t *)out.items;
    TEST_ASSERT_TRUE(actual[0] == 10 && actual[1] == 20 && actual[2] == 30);
    orc_sdk_arr_free(proxy.marks);
    TEST_ASSERT_TRUE(ORC_ERROR_NONE == orc_sdk_handle_free(&out));
    TEST_ASSERT_TRUE(ORC_ERROR_NONE == orc_sdk_handle_free(&in));
  }
  { /* i32 shuffle reverse. */
    OrcHandle in = {0}, out = {0};
    in.handle  = 1;
    out.handle = 2;
    orc_sdk_handle_alloc(ORC_TYPE_I32, &in);
    ORC_SDK_DECK_INIT(in.items, int32_t, (-1, -2, -3, -4));
    orc_sdk_oh_update(&in);
    OrcItemProxy *pdeck = NULL;
    TEST_ASSERT_TRUE(orc_sdk_deck_push(pdeck,
                                       ((OrcItemProxy) {.tree = 0, .item = 3}),
                                       1) == ORC_ERROR_NONE);
    TEST_ASSERT_TRUE(orc_sdk_deck_push(pdeck,
                                       ((OrcItemProxy) {.tree = 0, .item = 2}),
                                       0) == ORC_ERROR_NONE);
    TEST_ASSERT_TRUE(orc_sdk_deck_push(pdeck,
                                       ((OrcItemProxy) {.tree = 0, .item = 1}),
                                       0) == ORC_ERROR_NONE);
    TEST_ASSERT_TRUE(orc_sdk_deck_push(pdeck,
                                       ((OrcItemProxy) {.tree = 0, .item = 0}),
                                       0) == ORC_ERROR_NONE);
    OrcHandle proxy = _make_shuffle_proxy(pdeck);
    orc_sdk_deck_from_proxy(&in, 1, ORC_DECK_PROXY_SHUFFLE, &proxy, &out);
    TEST_ASSERT_TRUE(out.type_id == ORC_TYPE_I32);
    TEST_ASSERT_TRUE(orc_sdk_deck_len(out.items) == 4);
    int32_t *actual = (int32_t *)out.items;
    TEST_ASSERT_TRUE(actual[0] == -4 && actual[1] == -3 && actual[2] == -2 &&
                     actual[3] == -1);
    orc_sdk_deck_free(pdeck);
    TEST_ASSERT_TRUE(ORC_ERROR_NONE == orc_sdk_handle_free(&out));
    TEST_ASSERT_TRUE(ORC_ERROR_NONE == orc_sdk_handle_free(&in));
  }
  { /* i16 graft. */
    OrcHandle in = {0}, out = {0};
    in.handle  = 1;
    out.handle = 2;
    orc_sdk_handle_alloc(ORC_TYPE_I16, &in);
    ORC_SDK_DECK_INIT(in.items, int16_t, (10, 20, 30));
    orc_sdk_oh_update(&in);
    OrcHandle proxy = _make_grafted_proxy(in.items);
    orc_sdk_deck_from_proxy(&in, 1, ORC_DECK_PROXY_COPY_ITEMS, &proxy, &out);
    TEST_ASSERT_TRUE(out.type_id == ORC_TYPE_I16);
    TEST_ASSERT_TRUE(orc_sdk_deck_len(out.items) == 3);
    TEST_ASSERT_TRUE(orc_sdk_deck_max_depth(out.items) == 2);
    int16_t *actual = (int16_t *)out.items;
    TEST_ASSERT_TRUE(actual[0] == 10 && actual[1] == 20 && actual[2] == 30);
    orc_sdk_arr_free(proxy.marks);
    TEST_ASSERT_TRUE(ORC_ERROR_NONE == orc_sdk_handle_free(&out));
    TEST_ASSERT_TRUE(ORC_ERROR_NONE == orc_sdk_handle_free(&in));
  }
}

void setUp(void) {}
void tearDown(void) {}

int main(void)
{
  UNITY_BEGIN();
  RUN_TEST(test_arr_null_pointer_operations);
  RUN_TEST(test_arr_empty_array_operations);
  RUN_TEST(test_arr_index_boundary_conditions);
  RUN_TEST(test_orc_sdk_arr_capacity_management);
  RUN_TEST(test_arr_double_free_safety);
  RUN_TEST(test_arr_swap_remove_correctness);
  RUN_TEST(test_arr_memory_stress);
  RUN_TEST(test_arr_different_types);
  RUN_TEST(test_arr_ordered_remove);
  RUN_TEST(test_arr_resize_zero_fill);
  RUN_TEST(test_orc_sdk_arr_fill);
  RUN_TEST(test_orc_sdk_arr_clear);
  RUN_TEST(test_arr_remove_range);
  RUN_TEST(test_arr_pop);
  RUN_TEST(test_arr_fibonacci);
  RUN_TEST(test_arr_header_alignment);
  RUN_TEST(test_str_null_pointer_operations);
  RUN_TEST(test_str_push_basic);
  RUN_TEST(test_str_push_from_null);
  RUN_TEST(test_orc_str_remove_basic);
  RUN_TEST(test_orc_str_remove_boundary_conditions);
  RUN_TEST(test_orc_str_len_and_end);
  RUN_TEST(test_orc_str_free_and_reuse);
  RUN_TEST(test_str_push_special_characters);
  RUN_TEST(test_str_capacity_growth);
  RUN_TEST(test_str_mixed_operations);
  RUN_TEST(test_str_single_character);
  RUN_TEST(test_str_long_string);
  RUN_TEST(test_orc_str_clear_basic);
  RUN_TEST(test_orc_str_clear_null);
  RUN_TEST(test_orc_str_clear_and_reuse);
  RUN_TEST(test_orc_str_clear_already_empty);
  RUN_TEST(test_str_push_str_basic);
  RUN_TEST(test_str_push_str_to_null);
  RUN_TEST(test_str_push_str_empty_tail);
  RUN_TEST(test_str_push_str_empty_tail_to_null);
  RUN_TEST(test_str_push_str_multiple);
  RUN_TEST(test_str_push_str_after_remove);
  RUN_TEST(test_str_push_str_after_clear);
  RUN_TEST(test_str_push_str_long);
  RUN_TEST(test_str_push_str_single_char);
  RUN_TEST(test_orc_str_is_empty_null);
  RUN_TEST(test_orc_str_is_empty_after_operations);
  RUN_TEST(test_str_mixed_new_operations);
  RUN_TEST(test_orc_sv_from_str_and_basics);
  RUN_TEST(test_orc_sv_trim);
  RUN_TEST(test_orc_sv_split_at_delim);
  RUN_TEST(test_orc_sv_split_line);
  RUN_TEST(test_sv_trim_combined);
  RUN_TEST(test_orc_sv_starts_with);
  RUN_TEST(test_orc_sv_ends_with);
  RUN_TEST(test_orc_sv_contains_str);
  RUN_TEST(test_orc_sv_find);
  RUN_TEST(test_orc_sv_rfind);
  RUN_TEST(test_orc_str_eq);
  RUN_TEST(test_orc_sv_contains_char);
  RUN_TEST(test_orc_sv_strip_prefix);
  RUN_TEST(test_orc_sv_strip_suffix);
  RUN_TEST(test_orc_sv_slice);
  RUN_TEST(test_orc_sv_eq);
  RUN_TEST(test_orc_sdk_deck_header_alignment);
  RUN_TEST(test_deck_basic_push_and_length);
  RUN_TEST(test_deck_binary_deck);
  RUN_TEST(test_deck_mark_structure);
  RUN_TEST(test_orc_sdk_deck_clear);
  RUN_TEST(test_orc_sdk_deck_flatten);
  RUN_TEST(test_orc_sdk_deck_reserve);
  RUN_TEST(test_deck_depth_clamping);
  RUN_TEST(test_deck_single_element);
  RUN_TEST(test_orc_sdk_deck_free_null);
  RUN_TEST(test_deck_many_pushes);
  RUN_TEST(test_orc_sdk_deck_graft);
  RUN_TEST(test_orc_sdk_deck_simplify);
  RUN_TEST(test_deck_printf);
  RUN_TEST(test_deck_init);
  RUN_TEST(test_dv_binary_deck);
  RUN_TEST(test_dw_basic_depth2);
  RUN_TEST(test_dw_depth3_nested);
  RUN_TEST(test_dw_unbalanced_tree);
  RUN_TEST(test_dw_empty_groups);
  RUN_TEST(test_dw_nested_empty);
  RUN_TEST(test_dw_single_element_deep);
  RUN_TEST(test_orc_sdk_dw_len_tracking);
  RUN_TEST(test_dw_append_to_existing);
  RUN_TEST(test_dw_flat_depth1);
  RUN_TEST(test_dw_orc_sdk_deck_item_ptr);
  RUN_TEST(test_orc_sdk_dw_close_idempotent);
  RUN_TEST(test_dw_all_empty_depth3);
  RUN_TEST(test_orc_sdk_dims_equal);
  RUN_TEST(test_orc_sdk_dims_multiply);
  RUN_TEST(test_orc_sdk_dims_divide);
  RUN_TEST(test_orc_sdk_dims_pow);
  RUN_TEST(test_list_item_combinations);
  RUN_TEST(test_add_f64_combinations);
  RUN_TEST(test_list_length_combinations);
  RUN_TEST(test_two_output_combinations);
  RUN_TEST(test_first_add_combinations);
  RUN_TEST(test_deck_from_proxy_copy_items);
  RUN_TEST(test_deck_from_proxy_shuffle);
  RUN_TEST(test_deck_from_proxy_type_agnostic);
  RUN_TEST(test_registry_multithreaded);
  RUN_TEST(test_handle_alloc_uses_host_id);
  RUN_TEST(test_ensure_alloc_reuse);
  RUN_TEST(test_ensure_alloc_type_mismatch);
  RUN_TEST(test_ensure_alloc_fresh);
  RUN_TEST(test_ensure_alloc_eviction);
  RUN_TEST(test_hmap_basic);
  RUN_TEST(test_hmap_growth);
  RUN_TEST(test_hmap_edge_cases);
  RUN_TEST(test_hmap_null_operations);
  RUN_TEST(test_hmap_stress_test);
  RUN_TEST(test_hmap_hash_collision_simulation);
  RUN_TEST(test_hmap_boundary_conditions);
  RUN_TEST(test_hmap_extreme_values);
  RUN_TEST(test_hmap_repeated_growth);
  RUN_TEST(test_hmap_get_basic);
  RUN_TEST(test_hmap_get_after_updates);
  RUN_TEST(test_hmap_get_with_collisions);
  RUN_TEST(test_hmap_get_after_growth);
  RUN_TEST(test_hmap_get_edge_cases);
  RUN_TEST(test_hmap_get_null_safety);
  RUN_TEST(test_hmap_fibo_indices);
  RUN_TEST(test_hmap_remove_basic);
  RUN_TEST(test_hmap_remove_nonexistent);
  RUN_TEST(test_hmap_remove_returns_bool);
  RUN_TEST(test_hmap_remove_with_collisions);
  RUN_TEST(test_hmap_remove_after_growth);
  RUN_TEST(test_hmap_remove_and_reinsert);
  RUN_TEST(test_hmap_remove_null_safety);
  RUN_TEST(test_hmap_data_types_int8);
  RUN_TEST(test_hmap_data_types_int64);
  RUN_TEST(test_hmap_data_types_float);
  RUN_TEST(test_hmap_data_types_double);
  RUN_TEST(test_hmap_compaction_stress);
  RUN_TEST(test_hmap_hash_collision_stress);
  RUN_TEST(test_hmap_large_scale_operations);
  RUN_TEST(test_hmap_mixed_operation_patterns);
  RUN_TEST(test_hmap_header_alignment);
  RUN_TEST(test_hmap_is_empty);
  RUN_TEST(test_hmap_iterate_after_remove);
  RUN_TEST(test_hset_basic_operations);
  RUN_TEST(test_hset_remove_operations);
  RUN_TEST(test_hset_different_types);
  RUN_TEST(test_hset_large_scale);
  RUN_TEST(test_hset_edge_cases);
  RUN_TEST(test_hset_memory_operations);
  RUN_TEST(test_hset_is_empty_comprehensive);
  return UNITY_END();
}
