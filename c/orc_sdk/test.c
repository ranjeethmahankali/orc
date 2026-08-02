#include "orc_sdk.h"

#include <inttypes.h>
#include <limits.h>
#include <stddef.h>
#include <stdio.h>
#include <string.h>

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
  ORC_SDK_REQUIRE_WITH_MSG(orc_sdk_arr_len(null_arr) == 0,
                           "Null pointer represents an empty array");
  ORC_SDK_REQUIRE_WITH_MSG(orc_sdk_arr_end(null_arr) == null_arr,
                           "End of a NULL is itself");
  ORC_SDK_REQUIRE_WITH_MSG(orc_sdk_arr_swap_remove(null_arr, 0) == OUT_OF_BOUNDS,
                           "Cannot remove from empty array");
  double *arr = NULL;
  ORC_SDK_REQUIRE_WITH_MSG(orc_sdk_arr_reserve(arr, 10) == OK,
                           "Reserve starting with NULL");
  ORC_SDK_REQUIRE_WITH_MSG(arr != NULL, "Should be allocated after reserve");
  orc_sdk_arr_free(arr);
  // Should not crash
  double *ptr = NULL;
  orc_sdk_arr_free(ptr);
}

void test_arr_empty_array_operations(void)
{
  double *arr = NULL;
  ORC_SDK_REQUIRE_WITH_MSG(orc_sdk_arr_reserve(arr, 0) == OK, "Empty array reserve");
  ORC_SDK_REQUIRE_WITH_MSG(orc_sdk_arr_len(arr) == 0, "Length after reserved is zero");
  ORC_SDK_REQUIRE_WITH_MSG(orc_sdk_arr_swap_remove(arr, 0) == OUT_OF_BOUNDS,
                           "Cannot remove from empty array");
  ORC_SDK_REQUIRE_WITH_MSG(orc_sdk_arr_push(arr, 1.0) == OK, "Push into empty array");
  ORC_SDK_REQUIRE_WITH_MSG(orc_sdk_arr_len(arr) == 1, "Array after pushing");
  orc_sdk_arr_free(arr);
}

void test_arr_index_boundary_conditions(void)
{
  double *arr = NULL;
  orc_sdk_arr_push(arr, 1.0);
  // Single element array
  ORC_SDK_REQUIRE(orc_sdk_arr_swap_remove(arr, 0) == OK);
  ORC_SDK_REQUIRE(orc_sdk_arr_len(arr) == 0);
  ORC_SDK_REQUIRE(orc_sdk_arr_swap_remove(arr, 0) == OUT_OF_BOUNDS);  // Empty array
  // Add elements back
  orc_sdk_arr_push(arr, 1.0);
  orc_sdk_arr_push(arr, 2.0);
  ORC_SDK_REQUIRE(orc_sdk_arr_swap_remove(arr, orc_sdk_arr_len(arr)) ==
                  OUT_OF_BOUNDS);  // One past end
  ORC_SDK_REQUIRE(orc_sdk_arr_swap_remove(arr, orc_sdk_arr_len(arr) + 10) ==
                  OUT_OF_BOUNDS);  // Way past end
  ORC_SDK_REQUIRE(orc_sdk_arr_swap_remove(arr, SIZE_MAX) == OUT_OF_BOUNDS);  // Huge index
  orc_sdk_arr_free(arr);
}

void test_arr_capacity_management(void)
{
  double *arr = NULL;
  // Reserve initial capacity
  ORC_SDK_REQUIRE(orc_sdk_arr_reserve(arr, 4) == OK);
  ORC_SDK_REQUIRE(_orc_sdk_arr_capacity(arr) == 4);
  // Reserve smaller (should be no-op)
  size_t old_cap = _orc_sdk_arr_capacity(arr);
  ORC_SDK_REQUIRE(orc_sdk_arr_reserve(arr, 2) == OK);
  ORC_SDK_REQUIRE(_orc_sdk_arr_capacity(arr) == old_cap);
  // Reserve exact current capacity (should be no-op)
  ORC_SDK_REQUIRE(orc_sdk_arr_reserve(arr, old_cap) == OK);
  ORC_SDK_REQUIRE(_orc_sdk_arr_capacity(arr) == old_cap);
  // Reserve larger
  ORC_SDK_REQUIRE(orc_sdk_arr_reserve(arr, 10) == OK);
  ORC_SDK_REQUIRE(_orc_sdk_arr_capacity(arr) == 10);
  // Test growth pattern
  orc_sdk_arr_free(arr);
  arr = NULL;
  ORC_SDK_REQUIRE(orc_sdk_arr_push(arr, 1.0) == OK);
  size_t cap1 = _orc_sdk_arr_capacity(arr);
  // Fill to capacity
  while (orc_sdk_arr_len(arr) < cap1) {
    orc_sdk_arr_push(arr, (double)orc_sdk_arr_len(arr));
  }
  // Next push should grow
  orc_sdk_arr_push(arr, 999.0);
  ORC_SDK_REQUIRE(_orc_sdk_arr_capacity(arr) > cap1);
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
  ORC_SDK_REQUIRE(orc_sdk_arr_swap_remove(arr, 2) == OK);
  ORC_SDK_REQUIRE(orc_sdk_arr_len(arr) == 4);
  ORC_SDK_REQUIRE(arr[2] == 4.0);  // Last element moved here
  // Remove first element
  ORC_SDK_REQUIRE(orc_sdk_arr_swap_remove(arr, 0) == OK);
  ORC_SDK_REQUIRE(orc_sdk_arr_len(arr) == 3);
  ORC_SDK_REQUIRE(arr[0] == 3.0);  // Last element moved to front
  // Remove last element
  size_t last_idx = orc_sdk_arr_len(arr) - 1;
  ORC_SDK_REQUIRE(orc_sdk_arr_swap_remove(arr, last_idx) == OK);
  ORC_SDK_REQUIRE(orc_sdk_arr_len(arr) == 2);
  // Remove from single-element array
  orc_sdk_arr_swap_remove(arr, 0);
  orc_sdk_arr_swap_remove(arr, 0);
  ORC_SDK_REQUIRE(orc_sdk_arr_len(arr) == 0);
  orc_sdk_arr_free(arr);
}

void test_arr_memory_stress(void)
{
  double      *arr        = NULL;
  const size_t LARGE_SIZE = 1000;
  // Push many elements
  for (size_t i = 0; i < LARGE_SIZE; i++) {
    ORC_SDK_REQUIRE(orc_sdk_arr_push(arr, (double)i) == OK);
  }
  // Verify all elements
  ORC_SDK_REQUIRE(orc_sdk_arr_len(arr) == LARGE_SIZE);
  for (size_t i = 0; i < LARGE_SIZE; i++) {
    ORC_SDK_REQUIRE(arr[i] == (double)i);
  }
  // Repeated push/remove cycles
  for (int cycle = 0; cycle < 100; cycle++) {
    size_t old_len = orc_sdk_arr_len(arr);
    orc_sdk_arr_push(arr, 999.0);
    ORC_SDK_REQUIRE(orc_sdk_arr_len(arr) == old_len + 1);
    orc_sdk_arr_swap_remove(arr, orc_sdk_arr_len(arr) - 1);
    ORC_SDK_REQUIRE(orc_sdk_arr_len(arr) == old_len);
  }
  orc_sdk_arr_free(arr);
}

void test_arr_different_types(void)
{
  // Test with int
  int *ints = NULL;
  ORC_SDK_REQUIRE(orc_sdk_arr_push(ints, 42) == OK);
  ORC_SDK_REQUIRE(orc_sdk_arr_push(ints, -17) == OK);
  ORC_SDK_REQUIRE(orc_sdk_arr_len(ints) == 2);
  ORC_SDK_REQUIRE(ints[0] == 42);
  ORC_SDK_REQUIRE(ints[1] == -17);
  orc_sdk_arr_free(ints);
  // Test with char
  char *chars = NULL;
  ORC_SDK_REQUIRE(orc_sdk_arr_push(chars, 'A') == OK);
  ORC_SDK_REQUIRE(orc_sdk_arr_push(chars, 'B') == OK);
  ORC_SDK_REQUIRE(chars[0] == 'A');
  ORC_SDK_REQUIRE(chars[1] == 'B');
  orc_sdk_arr_free(chars);
  // Test with pointers
  const char  *strings[] = {"hello", "world"};
  const char **str_ptrs  = NULL;
  ORC_SDK_REQUIRE(orc_sdk_arr_push(str_ptrs, strings[0]) == OK);
  ORC_SDK_REQUIRE(orc_sdk_arr_push(str_ptrs, strings[1]) == OK);
  ORC_SDK_REQUIRE(str_ptrs[0] == strings[0]);
  ORC_SDK_REQUIRE(str_ptrs[1] == strings[1]);
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
  ORC_SDK_REQUIRE(orc_sdk_arr_push(points, p1) == OK);
  ORC_SDK_REQUIRE(orc_sdk_arr_push(points, p2) == OK);
  ORC_SDK_REQUIRE(points[0].x == 10);
  ORC_SDK_REQUIRE(points[0].y == 20);
  ORC_SDK_REQUIRE(points[0].value == 3.14);
  ORC_SDK_REQUIRE(points[1].x == -5);
  ORC_SDK_REQUIRE(points[1].y == 15);
  ORC_SDK_REQUIRE(points[1].value == 2.71);
  orc_sdk_arr_free(points);
  // Test alignment by checking data pointer alignment
  long long *longs = NULL;
  ORC_SDK_REQUIRE(orc_sdk_arr_push(longs, 123456789LL) == OK);
  // Check that data pointer is properly aligned for long long
  uintptr_t addr = (uintptr_t)longs;
  ORC_SDK_REQUIRE_WITH_MSG(addr % sizeof(long long) == 0,
                           "long long array not properly aligned");
  orc_sdk_arr_free(longs);
}

void test_arr_ordered_remove(void)
{
  int *arr = NULL;
  // Setup: [10, 20, 30, 40, 50]
  for (int i = 1; i <= 5; i++) {
    ORC_SDK_REQUIRE(orc_sdk_arr_push(arr, i * 10) == OK);
  }
  // Remove middle element (index 2, value 30)
  // Should shift [40, 50] left to fill the gap
  ORC_SDK_REQUIRE(orc_sdk_arr_remove(arr, 2) == OK);
  ORC_SDK_REQUIRE(orc_sdk_arr_len(arr) == 4);
  ORC_SDK_REQUIRE_WITH_MSG(arr[0] == 10, "First element should be unchanged");
  ORC_SDK_REQUIRE_WITH_MSG(arr[1] == 20, "Second element should be unchanged");
  ORC_SDK_REQUIRE_WITH_MSG(arr[2] == 40, "Third element should be 40 (was 4th)");
  ORC_SDK_REQUIRE_WITH_MSG(arr[3] == 50, "Fourth element should be 50 (was 5th)");
  // Remove first element
  // Should shift [20, 40, 50] left
  ORC_SDK_REQUIRE(orc_sdk_arr_remove(arr, 0) == OK);
  ORC_SDK_REQUIRE(orc_sdk_arr_len(arr) == 3);
  ORC_SDK_REQUIRE_WITH_MSG(arr[0] == 20, "First element should now be 20");
  ORC_SDK_REQUIRE_WITH_MSG(arr[1] == 40, "Second element should be 40");
  ORC_SDK_REQUIRE_WITH_MSG(arr[2] == 50, "Third element should be 50");
  // Remove last element
  // Should just decrease count, no shifting needed
  ORC_SDK_REQUIRE(orc_sdk_arr_remove(arr, 2) == OK);
  ORC_SDK_REQUIRE(orc_sdk_arr_len(arr) == 2);
  ORC_SDK_REQUIRE_WITH_MSG(arr[0] == 20, "First element unchanged");
  ORC_SDK_REQUIRE_WITH_MSG(arr[1] == 40, "Second element unchanged");
  // Remove from single-element array
  orc_sdk_arr_remove(arr, 0);
  orc_sdk_arr_remove(arr, 0);
  ORC_SDK_REQUIRE_WITH_MSG(orc_sdk_arr_len(arr) == 0, "Array should be empty");
  // Test bounds checking
  ORC_SDK_REQUIRE_WITH_MSG(orc_sdk_arr_remove(arr, 0) == OUT_OF_BOUNDS,
                           "Remove from empty array should fail");
  // Add one element and test invalid indices
  orc_sdk_arr_push(arr, 100);
  ORC_SDK_REQUIRE_WITH_MSG(orc_sdk_arr_remove(arr, 1) == OUT_OF_BOUNDS,
                           "Remove past end should fail");
  ORC_SDK_REQUIRE_WITH_MSG(orc_sdk_arr_remove(arr, SIZE_MAX) == OUT_OF_BOUNDS,
                           "Remove huge index should fail");
  orc_sdk_arr_free(arr);
  // Test with NULL pointer
  int *null_arr = NULL;
  ORC_SDK_REQUIRE_WITH_MSG(orc_sdk_arr_remove(null_arr, 0) == OUT_OF_BOUNDS,
                           "Remove from NULL should fail");
}

void test_arr_resize_zero_fill(void)
{
  double *arr = NULL;
  // Test resize from empty array - should zero-fill all elements
  orc_sdk_arr_resize(arr, 5);
  ORC_SDK_REQUIRE_WITH_MSG(arr != NULL, "Resize from empty should succeed");
  ORC_SDK_REQUIRE_WITH_MSG(orc_sdk_arr_len(arr) == 5, "Array should have 5 elements");
  for (size_t i = 0; i < 5; i++) {
    ORC_SDK_REQUIRE_WITH_MSG(arr[i] == 0.0, "All elements should be zero-initialized");
  }
  // Fill array with known values to test growth behavior
  for (size_t i = 0; i < 5; i++) {
    arr[i] = (double)(i + 10);  // [10, 11, 12, 13, 14]
  }
  // Test resize to larger size (growth) - new elements should be zero
  orc_sdk_arr_resize(arr, 8);
  ORC_SDK_REQUIRE_WITH_MSG(arr != NULL, "Resize growth should succeed");
  ORC_SDK_REQUIRE_WITH_MSG(orc_sdk_arr_len(arr) == 8, "Array should have 8 elements");
  ORC_SDK_REQUIRE_WITH_MSG(_orc_sdk_arr_capacity(arr) >= 8,
                           "Capacity should accommodate new size");
  // Check original elements unchanged
  for (size_t i = 0; i < 5; i++) {
    ORC_SDK_REQUIRE_WITH_MSG(arr[i] == (double)(i + 10),
                             "Original elements should be unchanged");
  }
  // Check new elements are zero-filled
  for (size_t i = 5; i < 8; i++) {
    ORC_SDK_REQUIRE_WITH_MSG(arr[i] == 0.0, "New elements should be zero-initialized");
  }
  // Test resize to smaller size (shrink) - remaining elements preserved
  orc_sdk_arr_resize(arr, 3);
  ORC_SDK_REQUIRE_WITH_MSG(arr != NULL, "Resize shrink should succeed");
  ORC_SDK_REQUIRE_WITH_MSG(orc_sdk_arr_len(arr) == 3, "Array should have 3 elements");
  // Capacity should remain the same (no reallocation on shrink)
  ORC_SDK_REQUIRE_WITH_MSG(_orc_sdk_arr_capacity(arr) >= 8,
                           "Capacity should not decrease on shrink");
  // Check remaining elements unchanged
  for (size_t i = 0; i < 3; i++) {
    ORC_SDK_REQUIRE_WITH_MSG(arr[i] == (double)(i + 10),
                             "Remaining elements should be unchanged");
  }
  // Test resize to same size (no-op)
  size_t len_before = orc_sdk_arr_len(arr);
  size_t cap_before = _orc_sdk_arr_capacity(arr);
  orc_sdk_arr_resize(arr, 3);
  ORC_SDK_REQUIRE_WITH_MSG(arr != NULL, "Resize same size should succeed");
  ORC_SDK_REQUIRE_WITH_MSG(orc_sdk_arr_len(arr) == len_before,
                           "Length should be unchanged");
  ORC_SDK_REQUIRE_WITH_MSG(_orc_sdk_arr_capacity(arr) == cap_before,
                           "Capacity should be unchanged");
  for (size_t i = 0; i < 3; i++) {
    ORC_SDK_REQUIRE_WITH_MSG(arr[i] == (double)(i + 10), "Elements should be unchanged");
  }
  // Test resize to zero (empty)
  orc_sdk_arr_resize(arr, 0);
  ORC_SDK_REQUIRE_WITH_MSG(arr != NULL, "Resize to zero should succeed");
  ORC_SDK_REQUIRE_WITH_MSG(orc_sdk_arr_len(arr) == 0, "Array should be empty");
  ORC_SDK_REQUIRE_WITH_MSG(_orc_sdk_arr_capacity(arr) >= 8,
                           "Capacity should be preserved");
  // Test resize from zero back to non-zero - should zero-fill
  orc_sdk_arr_resize(arr, 4);
  ORC_SDK_REQUIRE_WITH_MSG(arr != NULL, "Resize from zero should succeed");
  ORC_SDK_REQUIRE_WITH_MSG(orc_sdk_arr_len(arr) == 4, "Array should have 4 elements");
  for (size_t i = 0; i < 4; i++) {
    ORC_SDK_REQUIRE_WITH_MSG(arr[i] == 0.0, "Elements should be zero-initialized");
  }
  orc_sdk_arr_free(arr);
  // Test resize with NULL array - should create and zero-fill
  double *null_arr = NULL;
  orc_sdk_arr_resize(null_arr, 3);
  ORC_SDK_REQUIRE_WITH_MSG(null_arr != NULL, "Resize NULL array should succeed");
  ORC_SDK_REQUIRE_WITH_MSG(orc_sdk_arr_len(null_arr) == 3,
                           "Array should have 3 elements");
  for (size_t i = 0; i < 3; i++) {
    ORC_SDK_REQUIRE_WITH_MSG(null_arr[i] == 0.0, "Elements should be zero-initialized");
  }
  orc_sdk_arr_free(null_arr);
  // Test with different types to ensure zero-initialization works correctly
  int *int_arr = NULL;
  orc_sdk_arr_resize(int_arr, 3);
  ORC_SDK_REQUIRE_WITH_MSG(int_arr != NULL, "Int array resize should succeed");
  for (size_t i = 0; i < 3; i++) {
    ORC_SDK_REQUIRE_WITH_MSG(int_arr[i] == 0, "Int elements should be zero-initialized");
  }
  orc_sdk_arr_free(int_arr);
  // Test with pointers
  void **ptr_arr = NULL;
  orc_sdk_arr_resize(ptr_arr, 2);
  ORC_SDK_REQUIRE_WITH_MSG(ptr_arr != NULL, "Pointer array resize should succeed");
  for (size_t i = 0; i < 2; i++) {
    ORC_SDK_REQUIRE_WITH_MSG(ptr_arr[i] == NULL,
                             "Pointer elements should be NULL-initialized");
  }
  orc_sdk_arr_free(ptr_arr);
  // Test large resize to verify performance and correctness
  double      *large_arr  = NULL;
  const size_t LARGE_SIZE = 10000;
  orc_sdk_arr_resize(large_arr, LARGE_SIZE);
  ORC_SDK_REQUIRE_WITH_MSG(large_arr != NULL, "Large resize should succeed");
  ORC_SDK_REQUIRE_WITH_MSG(orc_sdk_arr_len(large_arr) == LARGE_SIZE,
                           "Large array should have correct length");
  // Spot check zero-initialization (checking all would be slow)
  ORC_SDK_REQUIRE_WITH_MSG(large_arr[0] == 0.0, "First element should be zero");
  ORC_SDK_REQUIRE_WITH_MSG(large_arr[LARGE_SIZE / 2] == 0.0,
                           "Middle element should be zero");
  ORC_SDK_REQUIRE_WITH_MSG(large_arr[LARGE_SIZE - 1] == 0.0,
                           "Last element should be zero");
  orc_sdk_arr_free(large_arr);
  // Test edge case: resize to 1 element
  double *single_arr = NULL;
  orc_sdk_arr_resize(single_arr, 1);
  ORC_SDK_REQUIRE_WITH_MSG(single_arr != NULL, "Single element resize should succeed");
  ORC_SDK_REQUIRE_WITH_MSG(orc_sdk_arr_len(single_arr) == 1,
                           "Array should have 1 element");
  ORC_SDK_REQUIRE_WITH_MSG(single_arr[0] == 0.0,
                           "Single element should be zero-initialized");
  // Modify the element then resize larger
  single_arr[0] = 42.0;
  orc_sdk_arr_resize(single_arr, 3);
  ORC_SDK_REQUIRE_WITH_MSG(single_arr[0] == 42.0, "Original element should be preserved");
  ORC_SDK_REQUIRE_WITH_MSG(single_arr[1] == 0.0,
                           "New elements should be zero-initialized");
  ORC_SDK_REQUIRE_WITH_MSG(single_arr[2] == 0.0,
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
  ORC_SDK_REQUIRE(orc_sdk_arr_len(ints) == 4);
  for (size_t i = 0; i < 4; i++) {
    ORC_SDK_REQUIRE_WITH_MSG(ints[i] == 42, "All elements should be 42");
  }
  // Test 2: Non-power of 2 size
  orc_sdk_arr_resize(ints, 7);
  val = 77;
  orc_sdk_arr_fill(ints, val);
  ORC_SDK_REQUIRE(orc_sdk_arr_len(ints) == 7);
  for (size_t i = 0; i < 7; i++) {
    ORC_SDK_REQUIRE_WITH_MSG(ints[i] == 77, "All elements should be 77");
  }
  // Test 3: Large size (to test doubling logic efficiency/correctness)
  orc_sdk_arr_resize(ints, 1025);
  val = 123;
  orc_sdk_arr_fill(ints, val);
  ORC_SDK_REQUIRE(orc_sdk_arr_len(ints) == 1025);
  for (size_t i = 0; i < 1025; i++) {
    ORC_SDK_REQUIRE_WITH_MSG(ints[i] == 123, "All elements should be 123");
  }
  orc_sdk_arr_free(ints);
  // Test 4: Single element
  double *doubles = NULL;
  orc_sdk_arr_resize(doubles, 1);
  double dval = 3.14;
  orc_sdk_arr_fill(doubles, dval);
  ORC_SDK_REQUIRE(orc_sdk_arr_len(doubles) == 1);
  ORC_SDK_REQUIRE(doubles[0] == 3.14);
  orc_sdk_arr_free(doubles);
  // Test 5: Empty array
  float *floats = NULL;
  dval          = 1.0f;
  orc_sdk_arr_fill(floats, dval);  // orc_sdk_arr_len(NULL) is 0
  ORC_SDK_REQUIRE(orc_sdk_arr_len(floats) == 0);
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
  ORC_SDK_REQUIRE(orc_sdk_arr_len(structs) == 3);
  for (size_t i = 0; i < 3; i++) {
    ORC_SDK_REQUIRE(structs[i].a == 10);
    ORC_SDK_REQUIRE(structs[i].b == 20.0);
  }
  orc_sdk_arr_free(structs);
}

void test_orc_sdk_arr_clear(void)
{
  double *arr = NULL;
  // Test clear on empty array
  orc_sdk_arr_clear(arr);
  ORC_SDK_REQUIRE_WITH_MSG(orc_sdk_arr_len(arr) == 0, "Clear on NULL array should work");
  // Add some elements
  orc_sdk_arr_resize(arr, 5);
  ORC_SDK_REQUIRE_WITH_MSG(orc_sdk_arr_len(arr) == 5, "Array should have 5 elements");
  size_t old_capacity = _orc_sdk_arr_capacity(arr);
  // Clear the array
  orc_sdk_arr_clear(arr);
  ORC_SDK_REQUIRE_WITH_MSG(orc_sdk_arr_len(arr) == 0,
                           "Array should be empty after clear");
  ORC_SDK_REQUIRE_WITH_MSG(_orc_sdk_arr_capacity(arr) == old_capacity,
                           "Capacity should be preserved");
  // Verify we can still use the array after clear
  OrcSdk_Status s = orc_sdk_arr_push(arr, 2.71);
  ORC_SDK_REQUIRE_WITH_MSG(s == OK, "Should be able to push after clear");
  ORC_SDK_REQUIRE_WITH_MSG(orc_sdk_arr_len(arr) == 1,
                           "Array should have 1 element after push");
  ORC_SDK_REQUIRE_WITH_MSG(arr[0] == 2.71, "Element should be correct");
  // Clear again with elements
  orc_sdk_arr_push(arr, 1.0);
  orc_sdk_arr_push(arr, 2.0);
  ORC_SDK_REQUIRE_WITH_MSG(orc_sdk_arr_len(arr) == 3, "Array should have 3 elements");
  orc_sdk_arr_clear(arr);
  ORC_SDK_REQUIRE_WITH_MSG(orc_sdk_arr_len(arr) == 0,
                           "Array should be empty after second clear");
  // Test operations on cleared array
  ORC_SDK_REQUIRE_WITH_MSG(orc_sdk_arr_swap_remove(arr, 0) == OUT_OF_BOUNDS,
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
    OrcSdk_Status s = orc_sdk_arr_push(arr, i * 10);
    ORC_SDK_REQUIRE_WITH_MSG(s == OK, "Setup should succeed");
  }
  ORC_SDK_REQUIRE_WITH_MSG(orc_sdk_arr_len(arr) == 5, "Array should have 5 elements");
  // Test 1: Remove middle range [1, 3) -> removes 20, 30
  // Expected: [10, 40, 50]
  OrcSdk_Status result = orc_sdk_arr_remove_range(arr, 1, 3);
  ORC_SDK_REQUIRE_WITH_MSG(result == OK, "Remove middle range should succeed");
  ORC_SDK_REQUIRE_WITH_MSG(orc_sdk_arr_len(arr) == 3,
                           "Array should have 3 elements after removing 2");
  ORC_SDK_REQUIRE_WITH_MSG(arr[0] == 10, "First element should be unchanged");
  ORC_SDK_REQUIRE_WITH_MSG(arr[1] == 40, "Second element should be 40 (was 4th)");
  ORC_SDK_REQUIRE_WITH_MSG(arr[2] == 50, "Third element should be 50 (was 5th)");
  // Test 2: Remove from beginning [0, 1) -> removes 10
  // Expected: [40, 50]
  result = orc_sdk_arr_remove_range(arr, 0, 1);
  ORC_SDK_REQUIRE_WITH_MSG(result == OK, "Remove from beginning should succeed");
  ORC_SDK_REQUIRE_WITH_MSG(orc_sdk_arr_len(arr) == 2, "Array should have 2 elements");
  ORC_SDK_REQUIRE_WITH_MSG(arr[0] == 40, "First element should be 40");
  ORC_SDK_REQUIRE_WITH_MSG(arr[1] == 50, "Second element should be 50");
  // Test 3: Remove from end [1, 2) -> removes 50
  // Expected: [40]
  result = orc_sdk_arr_remove_range(arr, 1, 2);
  ORC_SDK_REQUIRE_WITH_MSG(result == OK, "Remove from end should succeed");
  ORC_SDK_REQUIRE_WITH_MSG(orc_sdk_arr_len(arr) == 1, "Array should have 1 element");
  ORC_SDK_REQUIRE_WITH_MSG(arr[0] == 40, "Remaining element should be 40");
  // Test 4: Remove entire array [0, 1)
  result = orc_sdk_arr_remove_range(arr, 0, 1);
  ORC_SDK_REQUIRE_WITH_MSG(result == OK, "Remove entire array should succeed");
  ORC_SDK_REQUIRE_WITH_MSG(orc_sdk_arr_len(arr) == 0, "Array should be empty");
  // Test 5: Empty range operations
  // Add elements back
  orc_sdk_arr_push(arr, 100);
  orc_sdk_arr_push(arr, 200);
  orc_sdk_arr_push(arr, 300);
  ORC_SDK_REQUIRE_WITH_MSG(orc_sdk_arr_len(arr) == 3, "Array should have 3 elements");
  // Remove empty range at beginning [0, 0)
  result = orc_sdk_arr_remove_range(arr, 0, 0);
  ORC_SDK_REQUIRE_WITH_MSG(result == OK, "Empty range at beginning should succeed");
  ORC_SDK_REQUIRE_WITH_MSG(orc_sdk_arr_len(arr) == 3, "Array length should be unchanged");
  // Remove empty range in middle [1, 1)
  result = orc_sdk_arr_remove_range(arr, 1, 1);
  ORC_SDK_REQUIRE_WITH_MSG(result == OK, "Empty range in middle should succeed");
  ORC_SDK_REQUIRE_WITH_MSG(orc_sdk_arr_len(arr) == 3, "Array length should be unchanged");
  // Remove empty range at end [3, 3)
  result = orc_sdk_arr_remove_range(arr, 3, 3);
  ORC_SDK_REQUIRE_WITH_MSG(result == OK, "Empty range at end should succeed");
  ORC_SDK_REQUIRE_WITH_MSG(orc_sdk_arr_len(arr) == 3, "Array length should be unchanged");
  // Test 6: Error cases - out of bounds
  // Start index too large
  result = orc_sdk_arr_remove_range(arr, 4, 4);
  ORC_SDK_REQUIRE_WITH_MSG(result == OUT_OF_BOUNDS, "Start beyond array should fail");
  // Stop index too large
  result = orc_sdk_arr_remove_range(arr, 1, 5);
  ORC_SDK_REQUIRE_WITH_MSG(result == OUT_OF_BOUNDS, "Stop beyond array should fail");
  // Invalid range (stop < start)
  result = orc_sdk_arr_remove_range(arr, 2, 1);
  ORC_SDK_REQUIRE_WITH_MSG(result == OUT_OF_BOUNDS, "Invalid range should fail");
  // Test 7: Remove everything [0, length)
  result = orc_sdk_arr_remove_range(arr, 0, orc_sdk_arr_len(arr));
  ORC_SDK_REQUIRE_WITH_MSG(result == OK, "Remove all elements should succeed");
  ORC_SDK_REQUIRE_WITH_MSG(orc_sdk_arr_len(arr) == 0, "Array should be empty");
  orc_sdk_arr_free(arr);
  // Test 8: Operations on NULL array
  int *null_arr = NULL;
  result        = orc_sdk_arr_remove_range(null_arr, 0, 0);
  ORC_SDK_REQUIRE_WITH_MSG(result == OUT_OF_BOUNDS, "Remove from NULL array should fail");
  result = orc_sdk_arr_remove_range(null_arr, 0, 1);
  ORC_SDK_REQUIRE_WITH_MSG(result == OUT_OF_BOUNDS, "Remove from NULL array should fail");
  // Test 9: Large range removal
  int *large_arr = NULL;
  for (int i = 0; i < 10; i++) {
    orc_sdk_arr_push(large_arr, i);
  }
  ORC_SDK_REQUIRE_WITH_MSG(orc_sdk_arr_len(large_arr) == 10,
                           "Large array should have 10 elements");
  // Remove middle chunk [3, 7) -> removes 3, 4, 5, 6
  result = orc_sdk_arr_remove_range(large_arr, 3, 7);
  ORC_SDK_REQUIRE_WITH_MSG(result == OK, "Large range removal should succeed");
  ORC_SDK_REQUIRE_WITH_MSG(orc_sdk_arr_len(large_arr) == 6,
                           "Array should have 6 elements remaining");
  // Verify elements: should be [0, 1, 2, 7, 8, 9]
  int expected[] = {0, 1, 2, 7, 8, 9};
  for (size_t i = 0; i < 6; i++) {
    ORC_SDK_REQUIRE_WITH_MSG(large_arr[i] == expected[i],
                             "Large range removal elements should be correct");
  }
  orc_sdk_arr_free(large_arr);
}

void test_arr_pop(void)
{
  double *arr = NULL;
  double  value;
  // Test pop from empty array (should fail)
  OrcSdk_Status result = orc_sdk_arr_pop(arr, &value);
  ORC_SDK_REQUIRE_WITH_MSG(result == OUT_OF_BOUNDS, "Pop from empty array should fail");
  // Test pop from NULL array (should fail)
  double *null_arr = NULL;
  result           = orc_sdk_arr_pop(null_arr, &value);
  ORC_SDK_REQUIRE_WITH_MSG(result == OUT_OF_BOUNDS, "Pop from NULL array should fail");
  // Setup array with known values: [10.0, 20.0, 30.0]
  ORC_SDK_REQUIRE_WITH_MSG(orc_sdk_arr_push(arr, 10.0) == OK, "Push should succeed");
  ORC_SDK_REQUIRE_WITH_MSG(orc_sdk_arr_push(arr, 20.0) == OK, "Push should succeed");
  ORC_SDK_REQUIRE_WITH_MSG(orc_sdk_arr_push(arr, 30.0) == OK, "Push should succeed");
  ORC_SDK_REQUIRE_WITH_MSG(orc_sdk_arr_len(arr) == 3, "Array should have 3 elements");
  // Test pop from array with multiple elements
  result = orc_sdk_arr_pop(arr, &value);
  ORC_SDK_REQUIRE_WITH_MSG(result == OK, "Pop should succeed");
  ORC_SDK_REQUIRE_WITH_MSG(value == 30.0, "Popped value should be 30.0 (last element)");
  ORC_SDK_REQUIRE_WITH_MSG(orc_sdk_arr_len(arr) == 2,
                           "Array should have 2 elements after pop");
  ORC_SDK_REQUIRE_WITH_MSG(arr[0] == 10.0 && arr[1] == 20.0,
                           "Remaining elements should be correct");
  // Test sequential pops
  result = orc_sdk_arr_pop(arr, &value);
  ORC_SDK_REQUIRE_WITH_MSG(result == OK, "Second pop should succeed");
  ORC_SDK_REQUIRE_WITH_MSG(value == 20.0, "Popped value should be 20.0");
  ORC_SDK_REQUIRE_WITH_MSG(orc_sdk_arr_len(arr) == 1,
                           "Array should have 1 element after second pop");
  ORC_SDK_REQUIRE_WITH_MSG(arr[0] == 10.0, "Remaining element should be 10.0");
  // Test pop from single-element array
  result = orc_sdk_arr_pop(arr, &value);
  ORC_SDK_REQUIRE_WITH_MSG(result == OK, "Pop from single element should succeed");
  ORC_SDK_REQUIRE_WITH_MSG(value == 10.0, "Popped value should be 10.0");
  ORC_SDK_REQUIRE_WITH_MSG(orc_sdk_arr_len(arr) == 0,
                           "Array should be empty after popping last element");
  // Test pop from now-empty array (should fail)
  result = orc_sdk_arr_pop(arr, &value);
  ORC_SDK_REQUIRE_WITH_MSG(result == OUT_OF_BOUNDS, "Pop from empty array should fail");
  orc_sdk_arr_free(arr);
  // Test with different data types
  int *int_arr = NULL;
  int  int_value;
  ORC_SDK_REQUIRE_WITH_MSG(orc_sdk_arr_push(int_arr, 42) == OK,
                           "Int push should succeed");
  ORC_SDK_REQUIRE_WITH_MSG(orc_sdk_arr_push(int_arr, 99) == OK,
                           "Int push should succeed");
  result = orc_sdk_arr_pop(int_arr, &int_value);
  ORC_SDK_REQUIRE_WITH_MSG(result == OK, "Int pop should succeed");
  ORC_SDK_REQUIRE_WITH_MSG(int_value == 99, "Popped int value should be 99");
  ORC_SDK_REQUIRE_WITH_MSG(orc_sdk_arr_len(int_arr) == 1,
                           "Int array should have 1 element left");
  orc_sdk_arr_free(int_arr);
  // Test with pointers
  const char  *strings[] = {"first", "second", "third"};
  const char **str_arr   = NULL;
  const char  *str_value;
  ORC_SDK_REQUIRE_WITH_MSG(orc_sdk_arr_push(str_arr, strings[0]) == OK,
                           "String push should succeed");
  ORC_SDK_REQUIRE_WITH_MSG(orc_sdk_arr_push(str_arr, strings[1]) == OK,
                           "String push should succeed");
  ORC_SDK_REQUIRE_WITH_MSG(orc_sdk_arr_push(str_arr, strings[2]) == OK,
                           "String push should succeed");
  result = orc_sdk_arr_pop(str_arr, &str_value);
  ORC_SDK_REQUIRE_WITH_MSG(result == OK, "String pop should succeed");
  ORC_SDK_REQUIRE_WITH_MSG(str_value == strings[2], "Popped string should be 'third'");
  ORC_SDK_REQUIRE_WITH_MSG(orc_sdk_arr_len(str_arr) == 2,
                           "String array should have 2 elements left");
  orc_sdk_arr_free(str_arr);
  // Test capacity behavior - capacity should not decrease on pop
  double *cap_arr = NULL;
  ORC_SDK_REQUIRE(OK == orc_sdk_arr_reserve(cap_arr, 10));
  size_t initial_capacity = _orc_sdk_arr_capacity(cap_arr);
  // Fill with some elements
  for (int i = 0; i < 5; i++) {
    orc_sdk_arr_push(cap_arr, (double)i);
  }
  // Pop all elements
  for (int i = 0; i < 5; i++) {
    orc_sdk_arr_pop(cap_arr, &value);
  }
  ORC_SDK_REQUIRE_WITH_MSG(orc_sdk_arr_len(cap_arr) == 0, "Array should be empty");
  ORC_SDK_REQUIRE_WITH_MSG(_orc_sdk_arr_capacity(cap_arr) == initial_capacity,
                           "Capacity should not decrease");
  orc_sdk_arr_free(cap_arr);
  // Test push after pop (ensure array is still usable)
  double *reuse_arr = NULL;
  orc_sdk_arr_push(reuse_arr, 1.0);
  orc_sdk_arr_push(reuse_arr, 2.0);
  orc_sdk_arr_pop(reuse_arr, &value);
  ORC_SDK_REQUIRE_WITH_MSG(value == 2.0, "Popped value should be 2.0");
  orc_sdk_arr_push(reuse_arr, 3.0);
  ORC_SDK_REQUIRE_WITH_MSG(orc_sdk_arr_len(reuse_arr) == 2,
                           "Array should have 2 elements");
  ORC_SDK_REQUIRE_WITH_MSG(reuse_arr[0] == 1.0, "First element should be 1.0");
  ORC_SDK_REQUIRE_WITH_MSG(reuse_arr[1] == 3.0, "Second element should be 3.0");
  orc_sdk_arr_free(reuse_arr);
}

void test_arr_fibonacci(void)
{
  uint32_t *fibo = NULL;
  ORC_SDK_REQUIRE(orc_sdk_arr_push(fibo, 1) == OK);
  ORC_SDK_REQUIRE(orc_sdk_arr_push(fibo, 1) == OK);
  for (size_t i = 0; i < 10; ++i) {
    size_t const len = orc_sdk_arr_len(fibo);
    ORC_SDK_REQUIRE(orc_sdk_arr_push(fibo, fibo[len - 2] + fibo[len - 1]) == OK);
  }
  ORC_SDK_REQUIRE(orc_sdk_arr_len(fibo) == 12);
  uint32_t const expected[12] = {1, 1, 2, 3, 5, 8, 13, 21, 34, 55, 89, 144};
  for (size_t i = 0; i < 12; ++i) {
    ORC_SDK_REQUIRE(fibo[i] == expected[i]);
  }
  orc_sdk_arr_free(fibo);
}

void test_arr_header_alignment(void)
{
  ORC_SDK_REQUIRE_WITH_MSG(
    (sizeof(_OrcSdk_ArrHeader) % sizeof(_MaxAlignCompat)) == 0,
    "Array header must align with the platform's maximum alignment to be compatible "
    "with arbitrary types inside the container. This doesn't guarantee alignment with "
    "SIMD types. The containers are not meant to be used with SIMD types.");
}

// ============================================================================
// String tests
// ============================================================================

void test_str_null_pointer_operations(void)
{
  char *s = NULL;
  ORC_SDK_REQUIRE_WITH_MSG(orc_str_len(s) == 0, "Null string has length 0");
  ORC_SDK_REQUIRE_WITH_MSG(orc_str_end(s) == s, "End of NULL string is itself");
  ORC_SDK_REQUIRE_WITH_MSG(orc_str_remove(s, 0) == OUT_OF_BOUNDS,
                           "Cannot remove from NULL string");
  // Should not crash
  orc_str_free(s);
}

void test_str_push_basic(void)
{
  char *s = NULL;
  ORC_SDK_REQUIRE(orc_str_push(s, 'h') == OK);
  ORC_SDK_REQUIRE(orc_str_push(s, 'e') == OK);
  ORC_SDK_REQUIRE(orc_str_push(s, 'l') == OK);
  ORC_SDK_REQUIRE(orc_str_push(s, 'l') == OK);
  ORC_SDK_REQUIRE(orc_str_push(s, 'o') == OK);
  ORC_SDK_REQUIRE_WITH_MSG(orc_str_len(s) == 5, "String length should be 5");
  ORC_SDK_REQUIRE_WITH_MSG(strcmp(s, "hello") == 0, "String content should be 'hello'");
  ORC_SDK_REQUIRE_WITH_MSG(s[orc_str_len(s)] == '\0', "String must be null-terminated");
  orc_str_free(s);
}

void test_str_push_from_null(void)
{
  char *s = NULL;
  // First push allocates
  ORC_SDK_REQUIRE(orc_str_push(s, 'a') == OK);
  ORC_SDK_REQUIRE(s != NULL);
  ORC_SDK_REQUIRE(orc_str_len(s) == 1);
  ORC_SDK_REQUIRE(s[0] == 'a');
  ORC_SDK_REQUIRE(s[1] == '\0');
  // Subsequent pushes
  ORC_SDK_REQUIRE(orc_str_push(s, 'b') == OK);
  ORC_SDK_REQUIRE(orc_str_len(s) == 2);
  ORC_SDK_REQUIRE(strcmp(s, "ab") == 0);
  ORC_SDK_REQUIRE(orc_str_push(s, 'c') == OK);
  ORC_SDK_REQUIRE(orc_str_len(s) == 3);
  ORC_SDK_REQUIRE(strcmp(s, "abc") == 0);
  orc_str_free(s);
}

void test_orc_str_remove_basic(void)
{
  char *s = NULL;
  // Build "abcde"
  ORC_SDK_REQUIRE(orc_str_push(s, 'a') == OK);
  ORC_SDK_REQUIRE(orc_str_push(s, 'b') == OK);
  ORC_SDK_REQUIRE(orc_str_push(s, 'c') == OK);
  ORC_SDK_REQUIRE(orc_str_push(s, 'd') == OK);
  ORC_SDK_REQUIRE(orc_str_push(s, 'e') == OK);
  // Remove middle character 'c' at index 2
  ORC_SDK_REQUIRE(orc_str_remove(s, 2) == OK);
  ORC_SDK_REQUIRE(orc_str_len(s) == 4);
  ORC_SDK_REQUIRE(strcmp(s, "abde") == 0);
  ORC_SDK_REQUIRE(s[orc_str_len(s)] == '\0');
  // Remove first character 'a' at index 0
  ORC_SDK_REQUIRE(orc_str_remove(s, 0) == OK);
  ORC_SDK_REQUIRE(orc_str_len(s) == 3);
  ORC_SDK_REQUIRE(strcmp(s, "bde") == 0);
  ORC_SDK_REQUIRE(s[orc_str_len(s)] == '\0');
  // Remove last character 'e' at index 2
  ORC_SDK_REQUIRE(orc_str_remove(s, orc_str_len(s) - 1) == OK);
  ORC_SDK_REQUIRE(orc_str_len(s) == 2);
  ORC_SDK_REQUIRE(strcmp(s, "bd") == 0);
  ORC_SDK_REQUIRE(s[orc_str_len(s)] == '\0');
  orc_str_free(s);
}

void test_orc_str_remove_boundary_conditions(void)
{
  // Single character string
  char *s = NULL;
  ORC_SDK_REQUIRE(orc_str_push(s, 'x') == OK);
  ORC_SDK_REQUIRE(orc_str_remove(s, 0) == OK);
  ORC_SDK_REQUIRE_WITH_MSG(orc_str_len(s) == 0, "Empty after removing only character");
  ORC_SDK_REQUIRE_WITH_MSG(s[0] == '\0', "Still null-terminated when empty");
  // Remove from empty (non-NULL) string
  ORC_SDK_REQUIRE_WITH_MSG(orc_str_remove(s, 0) == OUT_OF_BOUNDS,
                           "Cannot remove from empty string");
  // One past end
  ORC_SDK_REQUIRE(orc_str_push(s, 'a') == OK);
  ORC_SDK_REQUIRE(orc_str_remove(s, orc_str_len(s)) == OUT_OF_BOUNDS);
  // Way past end
  ORC_SDK_REQUIRE(orc_str_remove(s, orc_str_len(s) + 10) == OUT_OF_BOUNDS);
  // Huge index
  ORC_SDK_REQUIRE(orc_str_remove(s, SIZE_MAX) == OUT_OF_BOUNDS);
  // Remove from NULL
  char *null_str = NULL;
  ORC_SDK_REQUIRE(orc_str_remove(null_str, 0) == OUT_OF_BOUNDS);
  orc_str_free(s);
}

void test_orc_str_len_and_end(void)
{
  char *s = NULL;
  ORC_SDK_REQUIRE(orc_str_len(s) == 0);
  // Build "abc"
  ORC_SDK_REQUIRE(orc_str_push(s, 'a') == OK);
  ORC_SDK_REQUIRE(orc_str_push(s, 'b') == OK);
  ORC_SDK_REQUIRE(orc_str_push(s, 'c') == OK);
  ORC_SDK_REQUIRE(orc_str_len(s) == 3);
  ORC_SDK_REQUIRE(orc_str_end(s) == s + orc_str_len(s));
  ORC_SDK_REQUIRE(*orc_str_end(s) == '\0');
  // After removal
  ORC_SDK_REQUIRE(orc_str_remove(s, 1) == OK);
  ORC_SDK_REQUIRE(orc_str_len(s) == 2);
  ORC_SDK_REQUIRE(orc_str_end(s) == s + orc_str_len(s));
  ORC_SDK_REQUIRE(*orc_str_end(s) == '\0');
  orc_str_free(s);
}

void test_orc_str_free_and_reuse(void)
{
  char *s = NULL;
  ORC_SDK_REQUIRE(orc_str_push(s, 'a') == OK);
  ORC_SDK_REQUIRE(orc_str_push(s, 'b') == OK);
  ORC_SDK_REQUIRE(orc_str_len(s) == 2);
  orc_str_free(s);
  ORC_SDK_REQUIRE_WITH_MSG(s == NULL, "Pointer is NULL after free");
  // Reuse after free
  ORC_SDK_REQUIRE(orc_str_push(s, 'x') == OK);
  ORC_SDK_REQUIRE(s != NULL);
  ORC_SDK_REQUIRE(orc_str_len(s) == 1);
  ORC_SDK_REQUIRE(strcmp(s, "x") == 0);
  orc_str_free(s);
}

void test_str_push_special_characters(void)
{
  char *s = NULL;
  // Whitespace characters
  ORC_SDK_REQUIRE(orc_str_push(s, ' ') == OK);
  ORC_SDK_REQUIRE(orc_str_push(s, '\t') == OK);
  ORC_SDK_REQUIRE(orc_str_push(s, '\n') == OK);
  ORC_SDK_REQUIRE(orc_str_len(s) == 3);
  ORC_SDK_REQUIRE(s[0] == ' ');
  ORC_SDK_REQUIRE(s[1] == '\t');
  ORC_SDK_REQUIRE(s[2] == '\n');
  ORC_SDK_REQUIRE(s[3] == '\0');
  orc_str_free(s);
  // Non-ASCII / high bytes
  s = NULL;
  ORC_SDK_REQUIRE(orc_str_push(s, (char)0xFF) == OK);
  ORC_SDK_REQUIRE(orc_str_push(s, (char)0x80) == OK);
  ORC_SDK_REQUIRE(orc_str_push(s, (char)0x01) == OK);
  ORC_SDK_REQUIRE(orc_str_len(s) == 3);
  ORC_SDK_REQUIRE(s[0] == (char)0xFF);
  ORC_SDK_REQUIRE(s[1] == (char)0x80);
  ORC_SDK_REQUIRE(s[2] == (char)0x01);
  ORC_SDK_REQUIRE(s[3] == '\0');
  orc_str_free(s);
  // Pushing a null byte - the header count still grows,
  // so orc_str_len reports based on header, not C string length
  s = NULL;
  ORC_SDK_REQUIRE(orc_str_push(s, 'a') == OK);
  ORC_SDK_REQUIRE(orc_str_push(s, '\0') == OK);
  ORC_SDK_REQUIRE(orc_str_push(s, 'b') == OK);
  ORC_SDK_REQUIRE_WITH_MSG(orc_str_len(s) == 3,
                           "orc_str_len tracks header count, not strlen");
  // But strlen would see only 1
  ORC_SDK_REQUIRE_WITH_MSG(strlen(s) == 1, "C strlen stops at embedded null");
  orc_str_free(s);
}

void test_str_capacity_growth(void)
{
  char        *s = NULL;
  size_t const n = 256;
  for (size_t i = 0; i < n; i++) {
    char ch = (char)('a' + (char)(i % 26));
    ORC_SDK_REQUIRE(orc_str_push(s, ch) == OK);
    ORC_SDK_REQUIRE(orc_str_len(s) == i + 1);
    ORC_SDK_REQUIRE(s[orc_str_len(s)] == '\0');
    // Capacity must be at least length + 1 (for null terminator)
    ORC_SDK_REQUIRE(_orc_sdk_arr_capacity(s) >= orc_str_len(s) + 1);
  }
  // Verify final content
  for (size_t i = 0; i < n; i++) {
    char expected = (char)('a' + (char)(i % 26));
    ORC_SDK_REQUIRE(s[i] == expected);
  }
  ORC_SDK_REQUIRE(orc_str_len(s) == n);
  orc_str_free(s);
}

void test_str_mixed_operations(void)
{
  char *s = NULL;
  // Build "hello"
  ORC_SDK_REQUIRE(orc_str_push(s, 'h') == OK);
  ORC_SDK_REQUIRE(orc_str_push(s, 'e') == OK);
  ORC_SDK_REQUIRE(orc_str_push(s, 'l') == OK);
  ORC_SDK_REQUIRE(orc_str_push(s, 'l') == OK);
  ORC_SDK_REQUIRE(orc_str_push(s, 'o') == OK);
  ORC_SDK_REQUIRE(strcmp(s, "hello") == 0);
  // Remove 'e' at index 1 -> "hllo"
  ORC_SDK_REQUIRE(orc_str_remove(s, 1) == OK);
  ORC_SDK_REQUIRE(strcmp(s, "hllo") == 0);
  ORC_SDK_REQUIRE(s[orc_str_len(s)] == '\0');
  // Push 'e' -> "hlloe"
  ORC_SDK_REQUIRE(orc_str_push(s, 'e') == OK);
  ORC_SDK_REQUIRE(strcmp(s, "hlloe") == 0);
  ORC_SDK_REQUIRE(s[orc_str_len(s)] == '\0');
  // Remove all characters one by one from front
  size_t len = orc_str_len(s);
  for (size_t i = 0; i < len; i++) {
    ORC_SDK_REQUIRE(orc_str_remove(s, 0) == OK);
  }
  ORC_SDK_REQUIRE(orc_str_len(s) == 0);
  ORC_SDK_REQUIRE(s[0] == '\0');
  // Push again after emptying
  ORC_SDK_REQUIRE(orc_str_push(s, 'z') == OK);
  ORC_SDK_REQUIRE(orc_str_len(s) == 1);
  ORC_SDK_REQUIRE(strcmp(s, "z") == 0);
  orc_str_free(s);
}

void test_str_single_character(void)
{
  char *s = NULL;
  ORC_SDK_REQUIRE(orc_str_push(s, 'x') == OK);
  ORC_SDK_REQUIRE(orc_str_len(s) == 1);
  ORC_SDK_REQUIRE(s[0] == 'x');
  ORC_SDK_REQUIRE(s[1] == '\0');
  ORC_SDK_REQUIRE(orc_str_end(s) == s + 1);
  // Remove it
  ORC_SDK_REQUIRE(orc_str_remove(s, 0) == OK);
  ORC_SDK_REQUIRE(orc_str_len(s) == 0);
  ORC_SDK_REQUIRE(s[0] == '\0');
  ORC_SDK_REQUIRE(orc_str_end(s) == s);
  // Push again - recover from empty
  ORC_SDK_REQUIRE(orc_str_push(s, 'y') == OK);
  ORC_SDK_REQUIRE(orc_str_len(s) == 1);
  ORC_SDK_REQUIRE(s[0] == 'y');
  ORC_SDK_REQUIRE(s[1] == '\0');
  orc_str_free(s);
}

void test_str_long_string(void)
{
  char        *s = NULL;
  size_t const n = 10000;
  // Build a long string
  for (size_t i = 0; i < n; i++) {
    ORC_SDK_REQUIRE(orc_str_push(s, (char)('A' + (char)(i % 26))) == OK);
  }
  ORC_SDK_REQUIRE(orc_str_len(s) == n);
  ORC_SDK_REQUIRE(s[n] == '\0');
  // Verify content
  for (size_t i = 0; i < n; i++) {
    ORC_SDK_REQUIRE(s[i] == (char)('A' + (char)(i % 26)));
  }
  // Remove 100 characters from the front
  for (size_t i = 0; i < 100; i++) {
    ORC_SDK_REQUIRE(orc_str_remove(s, 0) == OK);
  }
  ORC_SDK_REQUIRE(orc_str_len(s) == n - 100);
  ORC_SDK_REQUIRE(s[orc_str_len(s)] == '\0');
  // Remove from the end
  for (size_t i = 0; i < 100; i++) {
    ORC_SDK_REQUIRE(orc_str_remove(s, orc_str_len(s) - 1) == OK);
  }
  ORC_SDK_REQUIRE(orc_str_len(s) == n - 200);
  ORC_SDK_REQUIRE(s[orc_str_len(s)] == '\0');
  // Remove from middle
  size_t mid = orc_str_len(s) / 2;
  ORC_SDK_REQUIRE(orc_str_remove(s, mid) == OK);
  ORC_SDK_REQUIRE(orc_str_len(s) == n - 201);
  ORC_SDK_REQUIRE(s[orc_str_len(s)] == '\0');
  orc_str_free(s);
}

// orc_str_clear tests

void test_orc_str_clear_basic(void)
{
  char *s = NULL;
  ORC_SDK_REQUIRE(orc_str_push(s, 'a') == OK);
  ORC_SDK_REQUIRE(orc_str_push(s, 'b') == OK);
  ORC_SDK_REQUIRE(orc_str_push(s, 'c') == OK);
  ORC_SDK_REQUIRE(orc_str_len(s) == 3);
  orc_str_clear(s);
  ORC_SDK_REQUIRE_WITH_MSG(orc_str_len(s) == 0, "Length is 0 after clear");
  ORC_SDK_REQUIRE_WITH_MSG(s[0] == '\0', "Null-terminated after clear");
  ORC_SDK_REQUIRE_WITH_MSG(orc_str_is_empty(s), "String is empty after clear");
  // Capacity should be preserved
  ORC_SDK_REQUIRE(_orc_sdk_arr_capacity(s) >= 1);
  orc_str_free(s);
}

void test_orc_str_clear_null(void)
{
  // Should not crash
  char *s = NULL;
  orc_str_clear(s);
  ORC_SDK_REQUIRE(s == NULL);
}

void test_orc_str_clear_and_reuse(void)
{
  char *s = NULL;
  ORC_SDK_REQUIRE(orc_str_push(s, 'x') == OK);
  ORC_SDK_REQUIRE(orc_str_push(s, 'y') == OK);
  orc_str_clear(s);
  ORC_SDK_REQUIRE(orc_str_len(s) == 0);
  // Push after clear
  ORC_SDK_REQUIRE(orc_str_push(s, 'a') == OK);
  ORC_SDK_REQUIRE(orc_str_len(s) == 1);
  ORC_SDK_REQUIRE(strcmp(s, "a") == 0);
  // Clear and push again
  orc_str_clear(s);
  ORC_SDK_REQUIRE(orc_str_push(s, 'b') == OK);
  ORC_SDK_REQUIRE(orc_str_push(s, 'c') == OK);
  ORC_SDK_REQUIRE(strcmp(s, "bc") == 0);
  orc_str_free(s);
}

void test_orc_str_clear_already_empty(void)
{
  char *s = NULL;
  ORC_SDK_REQUIRE(orc_str_push(s, 'a') == OK);
  ORC_SDK_REQUIRE(orc_str_remove(s, 0) == OK);
  ORC_SDK_REQUIRE(orc_str_len(s) == 0);
  // Clear an already-empty (but allocated) string
  orc_str_clear(s);
  ORC_SDK_REQUIRE(orc_str_len(s) == 0);
  ORC_SDK_REQUIRE(s[0] == '\0');
  orc_str_free(s);
}

// orc_str_push_str tests

void test_str_push_str_basic(void)
{
  char *s = NULL;
  ORC_SDK_REQUIRE(orc_str_push(s, 'h') == OK);
  ORC_SDK_REQUIRE(orc_str_push(s, 'i') == OK);
  ORC_SDK_REQUIRE(orc_str_push_str(s, " world") == OK);
  ORC_SDK_REQUIRE_WITH_MSG(orc_str_len(s) == 8, "Length after push_str");
  ORC_SDK_REQUIRE_WITH_MSG(strcmp(s, "hi world") == 0, "Content after push_str");
  ORC_SDK_REQUIRE(s[orc_str_len(s)] == '\0');
  orc_str_free(s);
}

void test_str_push_str_to_null(void)
{
  // Push string onto NULL pointer
  char *s = NULL;
  ORC_SDK_REQUIRE(orc_str_push_str(s, "hello") == OK);
  ORC_SDK_REQUIRE(s != NULL);
  ORC_SDK_REQUIRE(orc_str_len(s) == 5);
  ORC_SDK_REQUIRE(strcmp(s, "hello") == 0);
  ORC_SDK_REQUIRE(s[orc_str_len(s)] == '\0');
  orc_str_free(s);
}

void test_str_push_str_empty_tail(void)
{
  // Push empty string onto existing string
  char *s = NULL;
  ORC_SDK_REQUIRE(orc_str_push_str(s, "abc") == OK);
  ORC_SDK_REQUIRE(orc_str_push_str(s, "") == OK);
  ORC_SDK_REQUIRE_WITH_MSG(orc_str_len(s) == 3,
                           "Length unchanged after pushing empty string");
  ORC_SDK_REQUIRE(strcmp(s, "abc") == 0);
  orc_str_free(s);
}

void test_str_push_str_empty_tail_to_null(void)
{
  // Push empty string onto NULL - should allocate an empty string, not fail
  char *s = NULL;
  ORC_SDK_REQUIRE_WITH_MSG(orc_str_push_str(s, "") == OK,
                           "Pushing empty to NULL should succeed");
  ORC_SDK_REQUIRE(s != NULL);
  ORC_SDK_REQUIRE(orc_str_len(s) == 0);
  ORC_SDK_REQUIRE(s[0] == '\0');
  orc_str_free(s);
}

void test_str_push_str_multiple(void)
{
  char *s = NULL;
  ORC_SDK_REQUIRE(orc_str_push_str(s, "foo") == OK);
  ORC_SDK_REQUIRE(orc_str_push_str(s, "bar") == OK);
  ORC_SDK_REQUIRE(orc_str_push_str(s, "baz") == OK);
  ORC_SDK_REQUIRE(orc_str_len(s) == 9);
  ORC_SDK_REQUIRE(strcmp(s, "foobarbaz") == 0);
  ORC_SDK_REQUIRE(s[orc_str_len(s)] == '\0');
  orc_str_free(s);
}

void test_str_push_str_after_remove(void)
{
  char *s = NULL;
  ORC_SDK_REQUIRE(orc_str_push_str(s, "abcde") == OK);
  // Remove middle character
  ORC_SDK_REQUIRE(orc_str_remove(s, 2) == OK);
  ORC_SDK_REQUIRE(strcmp(s, "abde") == 0);
  // Push string after removal
  ORC_SDK_REQUIRE(orc_str_push_str(s, "XY") == OK);
  ORC_SDK_REQUIRE(strcmp(s, "abdeXY") == 0);
  ORC_SDK_REQUIRE(orc_str_len(s) == 6);
  ORC_SDK_REQUIRE(s[orc_str_len(s)] == '\0');
  orc_str_free(s);
}

void test_str_push_str_after_clear(void)
{
  char *s = NULL;
  ORC_SDK_REQUIRE(orc_str_push_str(s, "hello") == OK);
  orc_str_clear(s);
  ORC_SDK_REQUIRE(orc_str_len(s) == 0);
  ORC_SDK_REQUIRE(orc_str_push_str(s, "world") == OK);
  ORC_SDK_REQUIRE(strcmp(s, "world") == 0);
  ORC_SDK_REQUIRE(orc_str_len(s) == 5);
  orc_str_free(s);
}

void test_str_push_str_long(void)
{
  char *s = NULL;
  // Build a long string by appending many times
  for (int i = 0; i < 500; i++) {
    ORC_SDK_REQUIRE(orc_str_push_str(s, "ab") == OK);
  }
  ORC_SDK_REQUIRE(orc_str_len(s) == 1000);
  ORC_SDK_REQUIRE(s[orc_str_len(s)] == '\0');
  // Verify pattern
  for (size_t i = 0; i < 1000; i += 2) {
    ORC_SDK_REQUIRE(s[i] == 'a');
    ORC_SDK_REQUIRE(s[i + 1] == 'b');
  }
  orc_str_free(s);
}

void test_str_push_str_single_char(void)
{
  // Push a single-character string (compare behavior with orc_str_push)
  char *s = NULL;
  ORC_SDK_REQUIRE(orc_str_push_str(s, "x") == OK);
  ORC_SDK_REQUIRE(orc_str_len(s) == 1);
  ORC_SDK_REQUIRE(strcmp(s, "x") == 0);
  // Equivalent to orc_str_push
  char *s2 = NULL;
  ORC_SDK_REQUIRE(orc_str_push(s2, 'x') == OK);
  ORC_SDK_REQUIRE(orc_str_len(s2) == 1);
  ORC_SDK_REQUIRE(strcmp(s, s2) == 0);
  orc_str_free(s);
  orc_str_free(s2);
}

// orc_str_is_empty tests

void test_orc_str_is_empty_null(void)
{
  char *s = NULL;
  ORC_SDK_REQUIRE_WITH_MSG(orc_str_is_empty(s), "NULL string is empty");
}

void test_orc_str_is_empty_after_operations(void)
{
  char *s = NULL;
  ORC_SDK_REQUIRE(orc_str_is_empty(s));
  ORC_SDK_REQUIRE(orc_str_push(s, 'a') == OK);
  ORC_SDK_REQUIRE_WITH_MSG(!orc_str_is_empty(s), "Non-empty after push");
  ORC_SDK_REQUIRE(orc_str_remove(s, 0) == OK);
  ORC_SDK_REQUIRE_WITH_MSG(orc_str_is_empty(s), "Empty after removing last char");
  ORC_SDK_REQUIRE(orc_str_push_str(s, "hi") == OK);
  ORC_SDK_REQUIRE(!orc_str_is_empty(s));
  orc_str_clear(s);
  ORC_SDK_REQUIRE_WITH_MSG(orc_str_is_empty(s), "Empty after clear");
  orc_str_free(s);
}

// Mixed operations across new and old API

void test_str_mixed_new_operations(void)
{
  char *s = NULL;
  ORC_SDK_REQUIRE(orc_str_push_str(s, "hello") == OK);
  ORC_SDK_REQUIRE(orc_str_push(s, '!') == OK);
  ORC_SDK_REQUIRE(strcmp(s, "hello!") == 0);
  orc_str_clear(s);
  ORC_SDK_REQUIRE(orc_str_is_empty(s));
  ORC_SDK_REQUIRE(orc_str_push(s, 'A') == OK);
  ORC_SDK_REQUIRE(orc_str_push_str(s, "BC") == OK);
  ORC_SDK_REQUIRE(strcmp(s, "ABC") == 0);
  ORC_SDK_REQUIRE(orc_str_remove(s, 1) == OK);
  ORC_SDK_REQUIRE(strcmp(s, "AC") == 0);
  ORC_SDK_REQUIRE(orc_str_push_str(s, "DE") == OK);
  ORC_SDK_REQUIRE(strcmp(s, "ACDE") == 0);
  ORC_SDK_REQUIRE(orc_str_len(s) == 4);
  ORC_SDK_REQUIRE(s[orc_str_len(s)] == '\0');
  orc_str_free(s);
}

// String view.

void test_orc_sv_from_str_and_basics(void)
{
  // From a normal string
  char       buf[] = "hello";
  OrcStrView sv    = orc_sv_from_str(buf);
  ORC_SDK_REQUIRE(sv.start == buf);
  ORC_SDK_REQUIRE(sv.end == buf + 5);
  ORC_SDK_REQUIRE(orc_sv_len(sv) == 5);
  ORC_SDK_REQUIRE(!orc_sv_is_empty(sv));
  // From empty string
  char       empty[] = "";
  OrcStrView e       = orc_sv_from_str(empty);
  ORC_SDK_REQUIRE(e.start == empty);
  ORC_SDK_REQUIRE(e.end == empty);
  ORC_SDK_REQUIRE(orc_sv_len(e) == 0);
  ORC_SDK_REQUIRE(orc_sv_is_empty(e));
  // From NULL
  OrcStrView n = orc_sv_from_str(NULL);
  ORC_SDK_REQUIRE(n.start == NULL);
  ORC_SDK_REQUIRE(n.end == NULL);
  ORC_SDK_REQUIRE(orc_sv_len(n) == 0);
  ORC_SDK_REQUIRE(orc_sv_is_empty(n));
}

void test_orc_sv_trim(void)
{
  // Trim left
  char       buf1[] = "  hi";
  OrcStrView sv1    = orc_sv_trim_left(orc_sv_from_str(buf1));
  ORC_SDK_REQUIRE(orc_sv_len(sv1) == 2);
  ORC_SDK_REQUIRE(memcmp(sv1.start, "hi", 2) == 0);
  // Trim right
  char       buf2[] = "hi  ";
  OrcStrView sv2    = orc_sv_trim_right(orc_sv_from_str(buf2));
  ORC_SDK_REQUIRE(orc_sv_len(sv2) == 2);
  ORC_SDK_REQUIRE(memcmp(sv2.start, "hi", 2) == 0);
  // Trim both
  char       buf3[] = " \t hi \n ";
  OrcStrView sv3    = orc_sv_trim_right(orc_sv_trim_left(orc_sv_from_str(buf3)));
  ORC_SDK_REQUIRE(orc_sv_len(sv3) == 2);
  ORC_SDK_REQUIRE(memcmp(sv3.start, "hi", 2) == 0);
  // All whitespace trims to empty
  char       buf4[] = "   ";
  OrcStrView sv4    = orc_sv_trim_left(orc_sv_from_str(buf4));
  ORC_SDK_REQUIRE(orc_sv_is_empty(sv4));
  char       buf5[] = "   ";
  OrcStrView sv5    = orc_sv_trim_right(orc_sv_from_str(buf5));
  ORC_SDK_REQUIRE(orc_sv_is_empty(sv5));
  // No whitespace is a no-op
  char       buf6[] = "abc";
  OrcStrView sv6    = orc_sv_trim_left(orc_sv_trim_right(orc_sv_from_str(buf6)));
  ORC_SDK_REQUIRE(orc_sv_len(sv6) == 3);
  ORC_SDK_REQUIRE(memcmp(sv6.start, "abc", 3) == 0);
  // Empty view
  OrcStrView sv7 = orc_sv_trim_left(orc_sv_from_str(""));
  ORC_SDK_REQUIRE(orc_sv_is_empty(sv7));
  OrcStrView sv8 = orc_sv_trim_right(orc_sv_from_str(""));
  ORC_SDK_REQUIRE(orc_sv_is_empty(sv8));
  // NULL view
  OrcStrView null_sv = orc_sv_trim_left((OrcStrView) {0});
  ORC_SDK_REQUIRE(null_sv.start == NULL);
  ORC_SDK_REQUIRE(null_sv.end == NULL);
  null_sv = orc_sv_trim_right((OrcStrView) {0});
  ORC_SDK_REQUIRE(null_sv.start == NULL);
  ORC_SDK_REQUIRE(null_sv.end == NULL);
}

void test_orc_sv_split_at_delim(void)
{
  // Basic split on comma
  char       buf[] = "one,two,three";
  OrcStrView sv    = orc_sv_from_str(buf);
  OrcStrView part1 = orc_sv_split_at_delim(&sv, ',');
  ORC_SDK_REQUIRE(orc_sv_len(part1) == 3);
  ORC_SDK_REQUIRE(memcmp(part1.start, "one", 3) == 0);
  ORC_SDK_REQUIRE_WITH_MSG(sv.start == buf + 4, "Remainder starts after delimiter");
  OrcStrView part2 = orc_sv_split_at_delim(&sv, ',');
  ORC_SDK_REQUIRE(orc_sv_len(part2) == 3);
  ORC_SDK_REQUIRE(memcmp(part2.start, "two", 3) == 0);
  // Last segment — no more delimiters, returns remainder and nulls out sv
  OrcStrView part3 = orc_sv_split_at_delim(&sv, ',');
  ORC_SDK_REQUIRE(orc_sv_len(part3) == 5);
  ORC_SDK_REQUIRE(memcmp(part3.start, "three", 5) == 0);
  ORC_SDK_REQUIRE(sv.start == NULL);
  ORC_SDK_REQUIRE(sv.end == NULL);
  // Splitting an exhausted view returns empty
  OrcStrView part4 = orc_sv_split_at_delim(&sv, ',');
  ORC_SDK_REQUIRE(orc_sv_is_empty(part4));
  // Delimiter at start yields empty first part
  char       buf2[] = ",hello";
  OrcStrView sv2    = orc_sv_from_str(buf2);
  OrcStrView first  = orc_sv_split_at_delim(&sv2, ',');
  ORC_SDK_REQUIRE(orc_sv_len(first) == 0);
  ORC_SDK_REQUIRE(orc_sv_len(sv2) == 5);
  ORC_SDK_REQUIRE(memcmp(sv2.start, "hello", 5) == 0);
  // Delimiter at end yields content then empty
  char       buf3[] = "hello,";
  OrcStrView sv3    = orc_sv_from_str(buf3);
  OrcStrView before = orc_sv_split_at_delim(&sv3, ',');
  ORC_SDK_REQUIRE(orc_sv_len(before) == 5);
  ORC_SDK_REQUIRE(memcmp(before.start, "hello", 5) == 0);
  OrcStrView after = orc_sv_split_at_delim(&sv3, ',');
  ORC_SDK_REQUIRE(orc_sv_len(after) == 0);
  ORC_SDK_REQUIRE(sv3.start == NULL);
  // No delimiter at all
  char       buf4[] = "none";
  OrcStrView sv4    = orc_sv_from_str(buf4);
  OrcStrView whole  = orc_sv_split_at_delim(&sv4, ',');
  ORC_SDK_REQUIRE(orc_sv_len(whole) == 4);
  ORC_SDK_REQUIRE(memcmp(whole.start, "none", 4) == 0);
  ORC_SDK_REQUIRE(sv4.start == NULL);
}

void test_orc_sv_split_line(void)
{
  char       buf[] = "line1\nline2\nline3";
  OrcStrView sv    = orc_sv_from_str(buf);
  OrcStrView l1    = orc_sv_split_line(&sv);
  ORC_SDK_REQUIRE(orc_sv_len(l1) == 5);
  ORC_SDK_REQUIRE(memcmp(l1.start, "line1", 5) == 0);
  OrcStrView l2 = orc_sv_split_line(&sv);
  ORC_SDK_REQUIRE(orc_sv_len(l2) == 5);
  ORC_SDK_REQUIRE(memcmp(l2.start, "line2", 5) == 0);
  OrcStrView l3 = orc_sv_split_line(&sv);
  ORC_SDK_REQUIRE(orc_sv_len(l3) == 5);
  ORC_SDK_REQUIRE(memcmp(l3.start, "line3", 5) == 0);
  ORC_SDK_REQUIRE(sv.start == NULL);
}

void test_sv_trim_combined(void)
{
  char       buf1[] = " \t hello \n ";
  OrcStrView sv1    = orc_sv_trim(orc_sv_from_str(buf1));
  ORC_SDK_REQUIRE(orc_sv_len(sv1) == 5);
  ORC_SDK_REQUIRE(memcmp(sv1.start, "hello", 5) == 0);
  // No whitespace
  char       buf2[] = "abc";
  OrcStrView sv2    = orc_sv_trim(orc_sv_from_str(buf2));
  ORC_SDK_REQUIRE(orc_sv_len(sv2) == 3);
  ORC_SDK_REQUIRE(memcmp(sv2.start, "abc", 3) == 0);
  // All whitespace
  char       buf3[] = "   ";
  OrcStrView sv3    = orc_sv_trim(orc_sv_from_str(buf3));
  ORC_SDK_REQUIRE(orc_sv_is_empty(sv3));
  // Empty and NULL
  ORC_SDK_REQUIRE(orc_sv_is_empty(orc_sv_trim(orc_sv_from_str(""))));
  ORC_SDK_REQUIRE(orc_sv_trim((OrcStrView) {0}).start == NULL);
}

void test_orc_sv_starts_with(void)
{
  char       buf[] = "hello world";
  OrcStrView sv    = orc_sv_from_str(buf);
  ORC_SDK_REQUIRE(orc_sv_starts_with(sv, "hello"));
  ORC_SDK_REQUIRE(orc_sv_starts_with(sv, "h"));
  ORC_SDK_REQUIRE(orc_sv_starts_with(sv, "hello world"));
  ORC_SDK_REQUIRE(!orc_sv_starts_with(sv, "hello world!"));
  ORC_SDK_REQUIRE(!orc_sv_starts_with(sv, "world"));
  ORC_SDK_REQUIRE(!orc_sv_starts_with(sv, "Hello"));
  // NULL prefix
  ORC_SDK_REQUIRE(!orc_sv_starts_with(sv, NULL));
  // Empty view
  OrcStrView empty = orc_sv_from_str("");
  ORC_SDK_REQUIRE(!orc_sv_starts_with(empty, "a"));
  // NULL view
  OrcStrView null_sv = (OrcStrView) {0};
  ORC_SDK_REQUIRE(!orc_sv_starts_with(null_sv, "a"));
  // Single char view
  char       buf2[] = "x";
  OrcStrView sv2    = orc_sv_from_str(buf2);
  ORC_SDK_REQUIRE(orc_sv_starts_with(sv2, "x"));
  ORC_SDK_REQUIRE(!orc_sv_starts_with(sv2, "xy"));
}

void test_orc_sv_ends_with(void)
{
  char       buf[] = "hello world";
  OrcStrView sv    = orc_sv_from_str(buf);
  ORC_SDK_REQUIRE(orc_sv_ends_with(sv, "world"));
  ORC_SDK_REQUIRE(orc_sv_ends_with(sv, "d"));
  ORC_SDK_REQUIRE(orc_sv_ends_with(sv, "hello world"));
  ORC_SDK_REQUIRE(!orc_sv_ends_with(sv, "hello world!"));
  ORC_SDK_REQUIRE(!orc_sv_ends_with(sv, "hello"));
  ORC_SDK_REQUIRE(!orc_sv_ends_with(sv, "World"));
  // NULL suffix
  ORC_SDK_REQUIRE(!orc_sv_ends_with(sv, NULL));
  // Empty view
  OrcStrView empty = orc_sv_from_str("");
  ORC_SDK_REQUIRE(!orc_sv_ends_with(empty, "a"));
  // NULL view
  OrcStrView null_sv = (OrcStrView) {0};
  ORC_SDK_REQUIRE(!orc_sv_ends_with(null_sv, "a"));
  // Single char view
  char       buf2[] = "x";
  OrcStrView sv2    = orc_sv_from_str(buf2);
  ORC_SDK_REQUIRE(orc_sv_ends_with(sv2, "x"));
  ORC_SDK_REQUIRE(!orc_sv_ends_with(sv2, "yx"));
}

void test_orc_sv_contains_str(void)
{
  char       buf[] = "hello world";
  OrcStrView sv    = orc_sv_from_str(buf);
  ORC_SDK_REQUIRE(orc_sv_contains_str(sv, "hello"));
  ORC_SDK_REQUIRE(orc_sv_contains_str(sv, "world"));
  ORC_SDK_REQUIRE(orc_sv_contains_str(sv, "lo wo"));
  ORC_SDK_REQUIRE(orc_sv_contains_str(sv, "hello world"));
  ORC_SDK_REQUIRE(orc_sv_contains_str(sv, "h"));
  ORC_SDK_REQUIRE(orc_sv_contains_str(sv, "d"));
  ORC_SDK_REQUIRE(!orc_sv_contains_str(sv, "hello world!"));
  ORC_SDK_REQUIRE(!orc_sv_contains_str(sv, "xyz"));
  ORC_SDK_REQUIRE(!orc_sv_contains_str(sv, "Hello"));
  // Repeated first-byte partial matches (regression: infinite loop)
  char       buf2[] = "aaab";
  OrcStrView sv2    = orc_sv_from_str(buf2);
  ORC_SDK_REQUIRE(orc_sv_contains_str(sv2, "aab"));
  ORC_SDK_REQUIRE(!orc_sv_contains_str(sv2, "aac"));
  // Empty needle
  ORC_SDK_REQUIRE(!orc_sv_contains_str(sv, ""));
  // NULL needle
  ORC_SDK_REQUIRE(!orc_sv_contains_str(sv, NULL));
  // Empty view
  OrcStrView empty = orc_sv_from_str("");
  ORC_SDK_REQUIRE(!orc_sv_contains_str(empty, "a"));
  // NULL view
  OrcStrView null_sv = (OrcStrView) {0};
  ORC_SDK_REQUIRE(!orc_sv_contains_str(null_sv, "a"));
  // Single char view
  char       buf3[] = "x";
  OrcStrView sv3    = orc_sv_from_str(buf3);
  ORC_SDK_REQUIRE(orc_sv_contains_str(sv3, "x"));
  ORC_SDK_REQUIRE(!orc_sv_contains_str(sv3, "y"));
  ORC_SDK_REQUIRE(!orc_sv_contains_str(sv3, "xy"));
  // Needle same length as view, no match
  char       buf4[] = "abc";
  OrcStrView sv4    = orc_sv_from_str(buf4);
  ORC_SDK_REQUIRE(!orc_sv_contains_str(sv4, "abd"));
  ORC_SDK_REQUIRE(orc_sv_contains_str(sv4, "abc"));
}

void test_orc_sv_find(void)
{
  char       buf[] = "hello";
  OrcStrView sv    = orc_sv_from_str(buf);
  ORC_SDK_REQUIRE(orc_sv_find(sv, 'h') == buf);
  ORC_SDK_REQUIRE(orc_sv_find(sv, 'o') == buf + 4);
  ORC_SDK_REQUIRE(orc_sv_find(sv, 'l') == buf + 2);
  ORC_SDK_REQUIRE(orc_sv_find(sv, 'z') == NULL);
  // Empty view
  OrcStrView empty = orc_sv_from_str("");
  ORC_SDK_REQUIRE(orc_sv_find(empty, 'a') == NULL);
  // NULL view
  OrcStrView null_sv = (OrcStrView) {0};
  ORC_SDK_REQUIRE(orc_sv_find(null_sv, 'a') == NULL);
  // Single char view
  char       buf2[] = "x";
  OrcStrView sv2    = orc_sv_from_str(buf2);
  ORC_SDK_REQUIRE(orc_sv_find(sv2, 'x') == buf2);
  ORC_SDK_REQUIRE(orc_sv_find(sv2, 'y') == NULL);
}

void test_orc_sv_rfind(void)
{
  char       buf[] = "hello";
  OrcStrView sv    = orc_sv_from_str(buf);
  // Finds last occurrence
  ORC_SDK_REQUIRE(orc_sv_rfind(sv, 'l') == buf + 3);
  ORC_SDK_REQUIRE(orc_sv_rfind(sv, 'h') == buf);
  ORC_SDK_REQUIRE(orc_sv_rfind(sv, 'o') == buf + 4);
  ORC_SDK_REQUIRE(orc_sv_rfind(sv, 'z') == NULL);
  // All same characters
  char       buf2[] = "aaaa";
  OrcStrView sv2    = orc_sv_from_str(buf2);
  ORC_SDK_REQUIRE(orc_sv_rfind(sv2, 'a') == buf2 + 3);
  // Empty view (regression: out-of-bounds dereference)
  OrcStrView empty = orc_sv_from_str("");
  ORC_SDK_REQUIRE(orc_sv_rfind(empty, 'a') == NULL);
  // NULL view
  OrcStrView null_sv = (OrcStrView) {0};
  ORC_SDK_REQUIRE(orc_sv_rfind(null_sv, 'a') == NULL);
  // Single char view
  char       buf3[] = "x";
  OrcStrView sv3    = orc_sv_from_str(buf3);
  ORC_SDK_REQUIRE(orc_sv_rfind(sv3, 'x') == buf3);
  ORC_SDK_REQUIRE(orc_sv_rfind(sv3, 'y') == NULL);
  // Only first char matches
  char       buf4[] = "abc";
  OrcStrView sv4    = orc_sv_from_str(buf4);
  ORC_SDK_REQUIRE(orc_sv_rfind(sv4, 'a') == buf4);
  // Only last char matches
  ORC_SDK_REQUIRE(orc_sv_rfind(sv4, 'c') == buf4 + 2);
}

void test_orc_str_eq(void)
{
  // Equal strings
  char *a = NULL;
  char *b = NULL;
  ORC_SDK_REQUIRE(orc_str_push_str(a, "hello") == OK);
  ORC_SDK_REQUIRE(orc_str_push_str(b, "hello") == OK);
  ORC_SDK_REQUIRE(orc_str_eq(a, b));
  // Different strings, same length
  orc_str_clear(b);
  ORC_SDK_REQUIRE(orc_str_push_str(b, "world") == OK);
  ORC_SDK_REQUIRE(!orc_str_eq(a, b));
  // Different lengths
  orc_str_clear(b);
  ORC_SDK_REQUIRE(orc_str_push_str(b, "hi") == OK);
  ORC_SDK_REQUIRE(!orc_str_eq(a, b));
  // Both NULL
  ORC_SDK_REQUIRE(orc_str_eq(NULL, NULL));
  // One NULL
  ORC_SDK_REQUIRE(!orc_str_eq(a, NULL));
  ORC_SDK_REQUIRE(!orc_str_eq(NULL, a));
  // Both empty
  orc_str_clear(a);
  orc_str_clear(b);
  ORC_SDK_REQUIRE(orc_str_eq(a, b));
  orc_str_free(a);
  orc_str_free(b);
}

void test_orc_sv_contains_char(void)
{
  char       buf[] = "hello";
  OrcStrView sv    = orc_sv_from_str(buf);
  ORC_SDK_REQUIRE(orc_sv_contains_char(sv, 'h'));
  ORC_SDK_REQUIRE(orc_sv_contains_char(sv, 'o'));
  ORC_SDK_REQUIRE(!orc_sv_contains_char(sv, 'z'));
  // Empty and NULL views
  ORC_SDK_REQUIRE(!orc_sv_contains_char(orc_sv_from_str(""), 'a'));
  ORC_SDK_REQUIRE(!orc_sv_contains_char((OrcStrView) {0}, 'a'));
}

void test_orc_sv_strip_prefix(void)
{
  char       buf[] = "hello world";
  OrcStrView sv    = orc_sv_from_str(buf);
  // Successful strip
  ORC_SDK_REQUIRE(orc_sv_strip_prefix(&sv, "hello"));
  ORC_SDK_REQUIRE(orc_sv_len(sv) == 6);
  ORC_SDK_REQUIRE(memcmp(sv.start, " world", 6) == 0);
  // Strip again on remainder
  ORC_SDK_REQUIRE(orc_sv_strip_prefix(&sv, " "));
  ORC_SDK_REQUIRE(orc_sv_len(sv) == 5);
  ORC_SDK_REQUIRE(memcmp(sv.start, "world", 5) == 0);
  // Prefix not present
  ORC_SDK_REQUIRE(!orc_sv_strip_prefix(&sv, "xyz"));
  ORC_SDK_REQUIRE_WITH_MSG(orc_sv_len(sv) == 5, "View unchanged on failed strip");
  // Prefix longer than view
  ORC_SDK_REQUIRE(!orc_sv_strip_prefix(&sv, "world!!!!"));
  // Strip entire view
  ORC_SDK_REQUIRE(orc_sv_strip_prefix(&sv, "world"));
  ORC_SDK_REQUIRE(orc_sv_is_empty(sv));
  // Empty view
  OrcStrView empty = orc_sv_from_str("");
  ORC_SDK_REQUIRE(!orc_sv_strip_prefix(&empty, "a"));
  // NULL view
  OrcStrView null_sv = (OrcStrView) {0};
  ORC_SDK_REQUIRE(!orc_sv_strip_prefix(&null_sv, "a"));
  // NULL prefix
  OrcStrView sv2 = orc_sv_from_str(buf);
  ORC_SDK_REQUIRE(!orc_sv_strip_prefix(&sv2, NULL));
  // NULL pointer to sv
  ORC_SDK_REQUIRE(!orc_sv_strip_prefix(NULL, "a"));
}

void test_orc_sv_strip_suffix(void)
{
  char       buf[] = "hello world";
  OrcStrView sv    = orc_sv_from_str(buf);
  // Successful strip
  ORC_SDK_REQUIRE(orc_sv_strip_suffix(&sv, "world"));
  ORC_SDK_REQUIRE(orc_sv_len(sv) == 6);
  ORC_SDK_REQUIRE(memcmp(sv.start, "hello ", 6) == 0);
  // Strip again on remainder
  ORC_SDK_REQUIRE(orc_sv_strip_suffix(&sv, " "));
  ORC_SDK_REQUIRE(orc_sv_len(sv) == 5);
  ORC_SDK_REQUIRE(memcmp(sv.start, "hello", 5) == 0);
  // Suffix not present
  ORC_SDK_REQUIRE(!orc_sv_strip_suffix(&sv, "xyz"));
  ORC_SDK_REQUIRE_WITH_MSG(orc_sv_len(sv) == 5, "View unchanged on failed strip");
  // Suffix longer than view
  ORC_SDK_REQUIRE(!orc_sv_strip_suffix(&sv, "!!!!hello"));
  // Strip entire view
  ORC_SDK_REQUIRE(orc_sv_strip_suffix(&sv, "hello"));
  ORC_SDK_REQUIRE(orc_sv_is_empty(sv));
  // Empty view
  OrcStrView empty = orc_sv_from_str("");
  ORC_SDK_REQUIRE(!orc_sv_strip_suffix(&empty, "a"));
  // NULL view
  OrcStrView null_sv = (OrcStrView) {0};
  ORC_SDK_REQUIRE(!orc_sv_strip_suffix(&null_sv, "a"));
  // NULL suffix
  OrcStrView sv2 = orc_sv_from_str(buf);
  ORC_SDK_REQUIRE(!orc_sv_strip_suffix(&sv2, NULL));
  // NULL pointer to sv
  ORC_SDK_REQUIRE(!orc_sv_strip_suffix(NULL, "a"));
}

void test_orc_sv_slice(void)
{
  char       buf[] = "hello world";
  OrcStrView sv    = orc_sv_from_str(buf);
  // Slice from middle
  OrcStrView mid = orc_sv_slice(sv, 2, 7);
  ORC_SDK_REQUIRE(orc_sv_len(mid) == 5);
  ORC_SDK_REQUIRE(memcmp(mid.start, "llo w", 5) == 0);
  // Slice from start
  OrcStrView head = orc_sv_slice(sv, 0, 5);
  ORC_SDK_REQUIRE(orc_sv_len(head) == 5);
  ORC_SDK_REQUIRE(memcmp(head.start, "hello", 5) == 0);
  // Slice to end
  OrcStrView tail = orc_sv_slice(sv, 6, 11);
  ORC_SDK_REQUIRE(orc_sv_len(tail) == 5);
  ORC_SDK_REQUIRE(memcmp(tail.start, "world", 5) == 0);
  // Full slice
  OrcStrView full = orc_sv_slice(sv, 0, 11);
  ORC_SDK_REQUIRE(orc_sv_len(full) == 11);
  ORC_SDK_REQUIRE(memcmp(full.start, "hello world", 11) == 0);
  // Empty slice (start == end)
  OrcStrView empty_slice = orc_sv_slice(sv, 3, 3);
  ORC_SDK_REQUIRE(orc_sv_is_empty(empty_slice));
  ORC_SDK_REQUIRE(empty_slice.start != NULL);
  // Single char slice
  OrcStrView one = orc_sv_slice(sv, 0, 1);
  ORC_SDK_REQUIRE(orc_sv_len(one) == 1);
  ORC_SDK_REQUIRE(*one.start == 'h');
  // Invalid: end > view length
  OrcStrView bad1 = orc_sv_slice(sv, 0, 100);
  ORC_SDK_REQUIRE(bad1.start == NULL);
  ORC_SDK_REQUIRE(bad1.end == NULL);
  // Invalid: start > end
  OrcStrView bad2 = orc_sv_slice(sv, 5, 2);
  ORC_SDK_REQUIRE(bad2.start == NULL);
  ORC_SDK_REQUIRE(bad2.end == NULL);
  // NULL view
  OrcStrView null_sv = (OrcStrView) {0};
  OrcStrView bad3    = orc_sv_slice(null_sv, 0, 1);
  ORC_SDK_REQUIRE(bad3.start == NULL);
}

void test_orc_sv_eq(void)
{
  char       buf1[] = "hello";
  char       buf2[] = "hello";
  OrcStrView a      = orc_sv_from_str(buf1);
  OrcStrView b      = orc_sv_from_str(buf2);
  // Equal views (different backing memory)
  ORC_SDK_REQUIRE(orc_sv_eq(a, b));
  // Same view
  ORC_SDK_REQUIRE(orc_sv_eq(a, a));
  // Different content, same length
  char       buf3[] = "world";
  OrcStrView c      = orc_sv_from_str(buf3);
  ORC_SDK_REQUIRE(!orc_sv_eq(a, c));
  // Different lengths
  char       buf4[] = "hi";
  OrcStrView d      = orc_sv_from_str(buf4);
  ORC_SDK_REQUIRE(!orc_sv_eq(a, d));
  // Both empty
  OrcStrView e1 = orc_sv_from_str("");
  OrcStrView e2 = orc_sv_from_str("");
  ORC_SDK_REQUIRE(orc_sv_eq(e1, e2));
  // Both NULL
  OrcStrView n1 = (OrcStrView) {0};
  OrcStrView n2 = (OrcStrView) {0};
  ORC_SDK_REQUIRE(orc_sv_eq(n1, n2));
  // One empty, one NULL (both have len 0)
  ORC_SDK_REQUIRE(orc_sv_eq(e1, n1));
  // Empty vs non-empty
  ORC_SDK_REQUIRE(!orc_sv_eq(e1, a));
  // Compare sub-slices
  OrcStrView sub    = orc_sv_slice(a, 0, 3);
  char       buf5[] = "hel";
  OrcStrView match  = orc_sv_from_str(buf5);
  ORC_SDK_REQUIRE(orc_sv_eq(sub, match));
}

void test_orc_sdk_deck_header_alignment(void)
{
  ORC_SDK_REQUIRE_WITH_MSG(
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
    ORC_SDK_REQUIRE(orc_sdk_deck_push(deck, i, tz) == OK);
  }
  return deck;
}

void test_deck_basic_push_and_length(void)
{
  size_t *deck = NULL;
  ORC_SDK_REQUIRE(orc_sdk_deck_len(deck) == 0);
  ORC_SDK_REQUIRE(orc_sdk_deck_max_depth(deck) == 0);
  ORC_SDK_REQUIRE(orc_sdk_deck_is_empty(deck));
  ORC_SDK_REQUIRE(orc_sdk_deck_push(deck, ((size_t) {7}), 1) == OK);
  ORC_SDK_REQUIRE(orc_sdk_deck_len(deck) == 1);
  ORC_SDK_REQUIRE(orc_sdk_deck_max_depth(deck) == 1);
  ORC_SDK_REQUIRE(!orc_sdk_deck_is_empty(deck));
  ORC_SDK_REQUIRE(_orc_sdk_deck_header(deck)->item_size == sizeof(size_t));
  ORC_SDK_REQUIRE(deck[0] == 7);
  ORC_SDK_REQUIRE(orc_sdk_deck_push(deck, ((size_t) {8}), 0) == OK);
  ORC_SDK_REQUIRE(orc_sdk_deck_push(deck, ((size_t) {9}), 0) == OK);
  ORC_SDK_REQUIRE(orc_sdk_deck_len(deck) == 3);
  ORC_SDK_REQUIRE(_orc_sdk_deck_header(deck)->item_size == sizeof(size_t));
  ORC_SDK_REQUIRE(deck[0] == 7);
  ORC_SDK_REQUIRE(deck[1] == 8);
  ORC_SDK_REQUIRE(deck[2] == 9);
  orc_sdk_deck_free(deck);
}

void test_deck_binary_deck(void)
{
  uint8_t const DEPTH = 5;
  size_t       *deck  = _binary_deck(DEPTH);
  ORC_SDK_REQUIRE(orc_sdk_deck_len(deck) == (size_t)(1 << DEPTH));
  ORC_SDK_REQUIRE(orc_sdk_deck_max_depth(deck) == DEPTH);
  ORC_SDK_REQUIRE(_orc_sdk_deck_header(deck)->item_size == sizeof(size_t));
  for (size_t i = 0; i < orc_sdk_deck_len(deck); ++i) {
    ORC_SDK_REQUIRE(deck[i] == i);
  }
  orc_sdk_deck_free(deck);
}

void test_deck_mark_structure(void)
{
  // Depth-3 binary deck: marks at positions 0,2,4,6 with depths 2,0,1,0.
  size_t             *deck = _binary_deck(3);
  _OrcSdk_DeckHeader *h    = _orc_sdk_deck_header(deck);
  ORC_SDK_REQUIRE(h->item_size == sizeof(size_t));
  ORC_SDK_REQUIRE(orc_sdk_arr_len(h->marks) == 4);
  ORC_SDK_REQUIRE(h->marks[0].depth == 2);
  ORC_SDK_REQUIRE(h->marks[1].depth == 0);
  ORC_SDK_REQUIRE(h->marks[2].depth == 1);
  ORC_SDK_REQUIRE(h->marks[3].depth == 0);
  ORC_SDK_REQUIRE(h->marks[0].pos == 0);
  ORC_SDK_REQUIRE(h->marks[1].pos == 2);
  ORC_SDK_REQUIRE(h->marks[2].pos == 4);
  ORC_SDK_REQUIRE(h->marks[3].pos == 6);
  orc_sdk_deck_free(deck);
}
void test_orc_sdk_deck_clear(void)
{
  size_t *deck = _binary_deck(3);
  ORC_SDK_REQUIRE(orc_sdk_deck_len(deck) == 8);
  ORC_SDK_REQUIRE(_orc_sdk_deck_header(deck)->item_size == sizeof(size_t));
  orc_sdk_deck_clear(deck);
  ORC_SDK_REQUIRE(orc_sdk_deck_len(deck) == 0);
  ORC_SDK_REQUIRE(orc_sdk_deck_max_depth(deck) == 0);
  ORC_SDK_REQUIRE(_orc_sdk_deck_header(deck)->item_size == sizeof(size_t));
  // Re-use after clear.
  ORC_SDK_REQUIRE(orc_sdk_deck_push(deck, ((size_t) {1}), 1) == OK);
  ORC_SDK_REQUIRE(orc_sdk_deck_push(deck, ((size_t) {2}), 0) == OK);
  ORC_SDK_REQUIRE(orc_sdk_deck_len(deck) == 2);
  ORC_SDK_REQUIRE(orc_sdk_deck_max_depth(deck) == 1);
  ORC_SDK_REQUIRE(_orc_sdk_deck_header(deck)->item_size == sizeof(size_t));
  ORC_SDK_REQUIRE(deck[0] == 1);
  ORC_SDK_REQUIRE(deck[1] == 2);
  orc_sdk_deck_free(deck);
}

void test_orc_sdk_deck_flatten(void)
{
  size_t *deck = _binary_deck(4);
  size_t  n    = orc_sdk_deck_len(deck);
  ORC_SDK_REQUIRE(n == 16);
  ORC_SDK_REQUIRE(_orc_sdk_deck_header(deck)->item_size == sizeof(size_t));
  orc_sdk_deck_flatten(deck);
  ORC_SDK_REQUIRE(orc_sdk_deck_max_depth(deck) == 1);
  ORC_SDK_REQUIRE(_orc_sdk_deck_header(deck)->item_size == sizeof(size_t));
  for (size_t i = 0; i < n; ++i) {
    ORC_SDK_REQUIRE(deck[i] == i);
  }
  _OrcSdk_DeckHeader *h = _orc_sdk_deck_header(deck);
  ORC_SDK_REQUIRE(orc_sdk_arr_len(h->marks) == 1);
  ORC_SDK_REQUIRE(h->marks[0].depth == 0);
  ORC_SDK_REQUIRE(h->marks[0].pos == 0);
  // Flatten is idempotent.
  orc_sdk_deck_flatten(deck);
  ORC_SDK_REQUIRE(orc_sdk_deck_max_depth(deck) == 1);
  ORC_SDK_REQUIRE(orc_sdk_arr_len(h->marks) == 1);
  orc_sdk_deck_free(deck);
  // Flatten a single-element deck pushed at depth 0: no marks.
  size_t *deck2 = NULL;
  ORC_SDK_REQUIRE(orc_sdk_deck_push(deck2, ((size_t) {5}), 0) == OK);
  ORC_SDK_REQUIRE(_orc_sdk_deck_header(deck2)->item_size == sizeof(size_t));
  orc_sdk_deck_flatten(deck2);
  ORC_SDK_REQUIRE(orc_sdk_deck_max_depth(deck2) == 0);
  ORC_SDK_REQUIRE(_orc_sdk_deck_header(deck2)->item_size == sizeof(size_t));
  ORC_SDK_REQUIRE(orc_sdk_arr_len(_orc_sdk_deck_header(deck2)->marks) == 0);
  orc_sdk_deck_free(deck2);
}

void test_orc_sdk_deck_reserve(void)
{
  size_t *deck = NULL;
  ORC_SDK_REQUIRE(orc_sdk_deck_reserve(deck, 32) == OK);
  ORC_SDK_REQUIRE(deck != NULL);
  ORC_SDK_REQUIRE(orc_sdk_deck_len(deck) == 0);
  ORC_SDK_REQUIRE(_orc_sdk_deck_header(deck)->item_size == sizeof(size_t));
  for (size_t i = 0; i < 32; ++i) {
    ORC_SDK_REQUIRE(orc_sdk_deck_push(deck, i, (i == 0) ? 1 : 0) == OK);
  }
  ORC_SDK_REQUIRE(orc_sdk_deck_len(deck) == 32);
  ORC_SDK_REQUIRE(_orc_sdk_deck_header(deck)->item_size == sizeof(size_t));
  for (size_t i = 0; i < 32; ++i) {
    ORC_SDK_REQUIRE(deck[i] == i);
  }
  orc_sdk_deck_free(deck);
}

void test_deck_depth_clamping(void)
{
  // Depth higher than the first mark's depth should be clamped.
  size_t *deck = NULL;
  ORC_SDK_REQUIRE(orc_sdk_deck_push(deck, ((size_t) {0}), 2) ==
                  OK);  // first mark: internal depth 1
  ORC_SDK_REQUIRE(orc_sdk_deck_push(deck, ((size_t) {1}), 0) == OK);
  ORC_SDK_REQUIRE(orc_sdk_deck_push(deck, ((size_t) {2}), 5) == OK);  // should be clamped
  ORC_SDK_REQUIRE(orc_sdk_deck_push(deck, ((size_t) {3}), 0) == OK);
  ORC_SDK_REQUIRE(orc_sdk_deck_len(deck) == 4);
  ORC_SDK_REQUIRE(orc_sdk_deck_max_depth(deck) == 2);
  _OrcSdk_DeckHeader *h = _orc_sdk_deck_header(deck);
  ORC_SDK_REQUIRE(h->item_size == sizeof(size_t));
  ORC_SDK_REQUIRE(orc_sdk_arr_len(h->marks) == 2);
  ORC_SDK_REQUIRE(h->marks[0].depth == 1);
  ORC_SDK_REQUIRE(h->marks[1].depth <= h->marks[0].depth);
  orc_sdk_deck_free(deck);
}

void test_deck_single_element(void)
{
  // Depth 0: bare leaf, no marks.
  size_t *deck = NULL;
  ORC_SDK_REQUIRE(orc_sdk_deck_push(deck, ((size_t) {42}), 0) == OK);
  ORC_SDK_REQUIRE(orc_sdk_deck_len(deck) == 1);
  ORC_SDK_REQUIRE(orc_sdk_deck_max_depth(deck) == 0);
  ORC_SDK_REQUIRE(_orc_sdk_deck_header(deck)->item_size == sizeof(size_t));
  ORC_SDK_REQUIRE(deck[0] == 42);
  ORC_SDK_REQUIRE(orc_sdk_arr_len(_orc_sdk_deck_header(deck)->marks) == 0);
  orc_sdk_deck_free(deck);
  // Depth 1.
  size_t *deck2 = NULL;
  ORC_SDK_REQUIRE(orc_sdk_deck_push(deck2, ((size_t) {7}), 1) == OK);
  ORC_SDK_REQUIRE(orc_sdk_deck_len(deck2) == 1);
  ORC_SDK_REQUIRE(orc_sdk_deck_max_depth(deck2) == 1);
  ORC_SDK_REQUIRE(_orc_sdk_deck_header(deck2)->item_size == sizeof(size_t));
  ORC_SDK_REQUIRE(deck2[0] == 7);
  orc_sdk_deck_free(deck2);
}

void test_orc_sdk_deck_free_null(void)
{
  size_t *deck = NULL;
  orc_sdk_deck_free(deck);
  ORC_SDK_REQUIRE(deck == NULL);
}

void test_deck_many_pushes(void)
{
  size_t *deck = NULL;
  size_t  n    = 1000;
  for (size_t i = 0; i < n; ++i) {
    ORC_SDK_REQUIRE(orc_sdk_deck_push(deck, i, (i == 0) ? 1 : 0) == OK);
  }
  ORC_SDK_REQUIRE(orc_sdk_deck_len(deck) == n);
  ORC_SDK_REQUIRE(orc_sdk_deck_max_depth(deck) == 1);
  ORC_SDK_REQUIRE(_orc_sdk_deck_header(deck)->item_size == sizeof(size_t));
  for (size_t i = 0; i < n; ++i) {
    ORC_SDK_REQUIRE(deck[i] == i);
  }
  orc_sdk_deck_free(deck);
}

void test_orc_sdk_deck_graft(void)
{
  // Graft a depth-3 binary deck: depth should increase by 1, items unchanged.
  size_t *deck = _binary_deck(3);
  ORC_SDK_REQUIRE(_orc_sdk_deck_header(deck)->item_size == sizeof(size_t));
  orc_sdk_deck_graft(deck);
  ORC_SDK_REQUIRE(orc_sdk_deck_max_depth(deck) == 4);
  ORC_SDK_REQUIRE(_orc_sdk_deck_header(deck)->item_size == sizeof(size_t));
  for (size_t i = 0; i < 8; ++i) {
    ORC_SDK_REQUIRE(deck[i] == i);
  }
  // After graft, every original item should have its own depth-0 mark,
  // plus the original marks with depth incremented by 1.
  // Original marks: depths [2,0,1,0] at positions [0,2,4,6].
  // After graft, we expect 8 marks total (one per item position), with:
  //   pos 0: depth 3, pos 1: depth 0, pos 2: depth 1, pos 3: depth 0,
  //   pos 4: depth 2, pos 5: depth 0, pos 6: depth 1, pos 7: depth 0.
  _OrcSdk_DeckHeader *h = _orc_sdk_deck_header(deck);
  ORC_SDK_REQUIRE(orc_sdk_arr_len(h->marks) == 8);
  uint8_t const expected_depths[] = {3, 0, 1, 0, 2, 0, 1, 0};
  for (size_t i = 0; i < 8; ++i) {
    ORC_SDK_REQUIRE(h->marks[i].depth == expected_depths[i]);
    ORC_SDK_REQUIRE(h->marks[i].pos == i);
  }
  orc_sdk_deck_free(deck);
  // Graft then flatten roundtrip: items survive.
  size_t *deck2 = _binary_deck(2);
  orc_sdk_deck_graft(deck2);
  ORC_SDK_REQUIRE(_orc_sdk_deck_header(deck2)->item_size == sizeof(size_t));
  orc_sdk_deck_flatten(deck2);
  ORC_SDK_REQUIRE(orc_sdk_deck_max_depth(deck2) == 1);
  ORC_SDK_REQUIRE(_orc_sdk_deck_header(deck2)->item_size == sizeof(size_t));
  ORC_SDK_REQUIRE(orc_sdk_deck_len(deck2) == 4);
  for (size_t i = 0; i < 4; ++i) {
    ORC_SDK_REQUIRE(deck2[i] == i);
  }
  orc_sdk_deck_free(deck2);
  // Graft a flat (depth-1) deck: each item gets wrapped.
  size_t *deck3 = NULL;
  for (size_t i = 0; i < 3; ++i) {
    ORC_SDK_REQUIRE(orc_sdk_deck_push(deck3, i, (i == 0) ? 1 : 0) == OK);
  }
  ORC_SDK_REQUIRE(orc_sdk_deck_max_depth(deck3) == 1);
  orc_sdk_deck_graft(deck3);
  ORC_SDK_REQUIRE(orc_sdk_deck_max_depth(deck3) == 2);
  ORC_SDK_REQUIRE(_orc_sdk_deck_header(deck3)->item_size == sizeof(size_t));
  ORC_SDK_REQUIRE(orc_sdk_deck_len(deck3) == 3);
  // Every item should have its own mark at depth 0 (wrapped individually),
  // plus the original depth-0 mark promoted to depth 1.
  _OrcSdk_DeckHeader *h3 = _orc_sdk_deck_header(deck3);
  ORC_SDK_REQUIRE(orc_sdk_arr_len(h3->marks) == 3);
  ORC_SDK_REQUIRE(h3->marks[0].depth == 1);
  ORC_SDK_REQUIRE(h3->marks[1].depth == 0);
  ORC_SDK_REQUIRE(h3->marks[2].depth == 0);
  for (size_t i = 0; i < 3; ++i) {
    ORC_SDK_REQUIRE(h3->marks[i].pos == i);
  }
  orc_sdk_deck_free(deck3);
  // Graft an empty deck: should be a no-op.
  size_t *deck4 = NULL;
  orc_sdk_deck_graft(deck4);
  ORC_SDK_REQUIRE(orc_sdk_deck_len(deck4) == 0);
}

void test_orc_sdk_deck_simplify(void)
{
  // A deck whose mark depths already use every level is unchanged.
  {
    size_t             *deck = _binary_deck(3);
    _OrcSdk_DeckHeader *h    = _orc_sdk_deck_header(deck);
    ORC_SDK_REQUIRE(h->item_size == sizeof(size_t));
    size_t const n_marks = orc_sdk_arr_len(h->marks);
    uint8_t      depths_before[4];
    for (size_t i = 0; i < n_marks; ++i)
      depths_before[i] = h->marks[i].depth;
    orc_sdk_deck_simplify(deck);
    for (size_t i = 0; i < n_marks; ++i) {
      ORC_SDK_REQUIRE(h->marks[i].depth == depths_before[i]);
    }
    orc_sdk_deck_free(deck);
  }
  // A deck with gaps in depth levels: only depths 0 and 4 present.
  // Should be remapped to 0 and 1.
  {
    size_t *deck = NULL;
    ORC_SDK_REQUIRE(orc_sdk_deck_push(deck, ((size_t) {0}), 5) == OK);  // mark depth = 4
    ORC_SDK_REQUIRE(orc_sdk_deck_push(deck, ((size_t) {1}), 0) == OK);
    ORC_SDK_REQUIRE(orc_sdk_deck_push(deck, ((size_t) {2}), 2) == OK);  // mark depth = 1
    ORC_SDK_REQUIRE(orc_sdk_deck_push(deck, ((size_t) {3}), 0) == OK);
    _OrcSdk_DeckHeader *h = _orc_sdk_deck_header(deck);
    ORC_SDK_REQUIRE(orc_sdk_arr_len(h->marks) == 2);
    // Before simplify: depths are 4 and 1 (clamped from external 5 and 2).
    // Wait — first mark has internal depth 4, second gets clamped to 4.
    // But external 2 -> internal 1, which is <= 4, so no clamping.
    ORC_SDK_REQUIRE(h->marks[0].depth == 4);
    ORC_SDK_REQUIRE(h->marks[1].depth == 1);
    orc_sdk_deck_simplify(deck);
    ORC_SDK_REQUIRE(orc_sdk_deck_max_depth(deck) == 2);
    ORC_SDK_REQUIRE(h->item_size == sizeof(size_t));
    ORC_SDK_REQUIRE(h->marks[0].depth == 1);
    ORC_SDK_REQUIRE(h->marks[1].depth == 0);
    // Items unchanged.
    for (size_t i = 0; i < 4; ++i) {
      ORC_SDK_REQUIRE(deck[i] == i);
    }
    orc_sdk_deck_free(deck);
  }
  // Simplify is idempotent.
  {
    size_t *deck = NULL;
    ORC_SDK_REQUIRE(orc_sdk_deck_push(deck, ((size_t) {0}), 5) == OK);
    ORC_SDK_REQUIRE(orc_sdk_deck_push(deck, ((size_t) {1}), 0) == OK);
    ORC_SDK_REQUIRE(orc_sdk_deck_push(deck, ((size_t) {2}), 2) == OK);
    ORC_SDK_REQUIRE(orc_sdk_deck_push(deck, ((size_t) {3}), 0) == OK);
    orc_sdk_deck_simplify(deck);
    _OrcSdk_DeckHeader *h  = _orc_sdk_deck_header(deck);
    uint8_t const       d0 = h->marks[0].depth;
    uint8_t const       d1 = h->marks[1].depth;
    orc_sdk_deck_simplify(deck);
    ORC_SDK_REQUIRE(h->marks[0].depth == d0);
    ORC_SDK_REQUIRE(h->marks[1].depth == d1);
    orc_sdk_deck_free(deck);
  }
  // Simplify after graft.
  {
    size_t *deck = _binary_deck(3);
    orc_sdk_deck_graft(deck);
    ORC_SDK_REQUIRE(orc_sdk_deck_max_depth(deck) == 4);
    // After graft, depths are contiguous (0,1,2,3), so simplify is a no-op.
    _OrcSdk_DeckHeader *h             = _orc_sdk_deck_header(deck);
    size_t const        n_marks       = orc_sdk_arr_len(h->marks);
    uint8_t            *depths_before = NULL;
    for (size_t i = 0; i < n_marks; ++i) {
      orc_sdk_arr_push(depths_before, h->marks[i].depth);
    }
    orc_sdk_deck_simplify(deck);
    ORC_SDK_REQUIRE(orc_sdk_deck_max_depth(deck) == 4);
    for (size_t i = 0; i < n_marks; ++i) {
      ORC_SDK_REQUIRE(h->marks[i].depth == depths_before[i]);
    }
    orc_sdk_arr_free(depths_before);
    orc_sdk_deck_free(deck);
  }
  // Simplify on empty/NULL deck is safe.
  {
    size_t *deck = NULL;
    orc_sdk_deck_simplify(deck);
    ORC_SDK_REQUIRE(orc_sdk_deck_len(deck) == 0);
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
  ORC_SDK_REQUIRE(_orc_sdk_deck_header(deck)->item_size == sizeof(size_t));
  char *output = orc_sdk_deck_to_str(deck, _print_size_t);
  ORC_SDK_REQUIRE(orc_sv_eq(orc_sv_trim(orc_sv_from_str(output)),
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
  ORC_SDK_REQUIRE(orc_sv_eq(orc_sv_trim(orc_sv_from_str(output)),
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
  ORC_SDK_REQUIRE(orc_sv_eq(orc_sv_from_str(output),
                            orc_sv_from_str("  1 ---| 10\n"
                                            "       | 20\n"
                                            "       | 30\n")));
  orc_str_free(output);
  // List with single element.
  ORC_SDK_DECK_INIT(deck, size_t, (42));
  output = orc_sdk_deck_to_str(deck, _print_size_t);
  ORC_SDK_REQUIRE(orc_sv_eq(orc_sv_from_str(output), orc_sv_from_str("  1 ---| 42\n")));
  orc_str_free(output);
  // All empty depth-2.
  ORC_SDK_DECK_INIT(deck, size_t, ((), (), ()));
  output = orc_sdk_deck_to_str(deck, _print_size_t);
  ORC_SDK_REQUIRE(orc_sv_eq(orc_sv_from_str(output),
                            orc_sv_from_str("  2 ------|\n"
                                            "     1 ---|\n"
                                            "     1 ---|\n")));
  orc_str_free(output);
  // Empty deck.
  orc_sdk_deck_clear(deck);
  output = orc_sdk_deck_to_str(deck, _print_size_t);
  ORC_SDK_REQUIRE(output == NULL);
  // Depth-3 with nested empty.
  ORC_SDK_DECK_INIT(deck, size_t, (((1, 2), ()), (()), ((3))));
  output = orc_sdk_deck_to_str(deck, _print_size_t);
  ORC_SDK_REQUIRE(orc_sv_eq(orc_sv_from_str(output),
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
  ORC_SDK_REQUIRE(orc_sdk_deck_len(deck) == 3);
  ORC_SDK_REQUIRE(orc_sdk_deck_max_depth(deck) == 1);
  ORC_SDK_REQUIRE(_orc_sdk_deck_header(deck)->item_size == sizeof(size_t));
  ORC_SDK_REQUIRE(deck[0] == 10);
  ORC_SDK_REQUIRE(deck[1] == 20);
  ORC_SDK_REQUIRE(deck[2] == 30);
  h = _orc_sdk_deck_header(deck);
  ORC_SDK_REQUIRE(orc_sdk_arr_len(h->marks) == 1);
  ORC_SDK_REQUIRE(h->marks[0].depth == 0);
  ORC_SDK_REQUIRE(h->marks[0].pos == 0);
  /* depth 2: re-init clears existing data */
  ORC_SDK_DECK_INIT(deck, size_t, ((1, 2, 3), (4, 5, 6), (7, 8, 9)));
  ORC_SDK_REQUIRE(orc_sdk_deck_len(deck) == 9);
  ORC_SDK_REQUIRE(orc_sdk_deck_max_depth(deck) == 2);
  ORC_SDK_REQUIRE(_orc_sdk_deck_header(deck)->item_size == sizeof(size_t));
  for (size_t i = 0; i < 9; ++i)
    ORC_SDK_REQUIRE(deck[i] == i + 1);
  h = _orc_sdk_deck_header(deck);
  ORC_SDK_REQUIRE(orc_sdk_arr_len(h->marks) == 3);
  ORC_SDK_REQUIRE(h->marks[0].depth == 1);
  ORC_SDK_REQUIRE(h->marks[0].pos == 0);
  ORC_SDK_REQUIRE(h->marks[1].depth == 0);
  ORC_SDK_REQUIRE(h->marks[1].pos == 3);
  ORC_SDK_REQUIRE(h->marks[2].depth == 0);
  ORC_SDK_REQUIRE(h->marks[2].pos == 6);
  /* depth 3: ruler sequence depths 2,0,1,0 at positions 0,2,4,6 */
  ORC_SDK_DECK_INIT(deck, size_t, (((1, 2), (3, 4)), ((5, 6), (7, 8))));
  ORC_SDK_REQUIRE(orc_sdk_deck_len(deck) == 8);
  ORC_SDK_REQUIRE(orc_sdk_deck_max_depth(deck) == 3);
  ORC_SDK_REQUIRE(_orc_sdk_deck_header(deck)->item_size == sizeof(size_t));
  for (size_t i = 0; i < 8; ++i)
    ORC_SDK_REQUIRE(deck[i] == i + 1);
  h = _orc_sdk_deck_header(deck);
  ORC_SDK_REQUIRE(orc_sdk_arr_len(h->marks) == 4);
  ORC_SDK_REQUIRE(h->marks[0].depth == 2);
  ORC_SDK_REQUIRE(h->marks[0].pos == 0);
  ORC_SDK_REQUIRE(h->marks[1].depth == 0);
  ORC_SDK_REQUIRE(h->marks[1].pos == 2);
  ORC_SDK_REQUIRE(h->marks[2].depth == 1);
  ORC_SDK_REQUIRE(h->marks[2].pos == 4);
  ORC_SDK_REQUIRE(h->marks[3].depth == 0);
  ORC_SDK_REQUIRE(h->marks[3].pos == 6);
  orc_sdk_deck_free(deck);
}

// ========== OrcSdk_DeckView ==========

void _print_deck_view(OrcSdk_DeckView const *v)  // DEBUG
{
  fprintf(stderr,
          "\nOrcSdk_DeckView {\n"
          "  items:         %p;\n"
          "  n_items:       %zu;\n"
          "  item_size:     %zu;\n"
          "  marks:         %p;\n"
          "  stride_offset: %p;\n"
          "  n_marks:       %zu;\n"
          "  strides:       %p;\n"
          "  depth:         %d;\n"
          "  start:         %zu;\n"
          "  end:           %zu;\n"
          "}\n",
          (void *)v->items,
          v->n_items,
          v->item_size,
          (void *)v->marks,
          (void *)v->stride_offset,
          v->n_marks,
          (void *)v->strides,
          v->depth,
          v->start,
          v->end);
}

void test_dv_binary_deck(void)
{
  const uint8_t DEPTH = 5;
  size_t       *deck  = _binary_deck(DEPTH);
  ORC_SDK_REQUIRE(DEPTH == orc_sdk_deck_max_depth(deck));
  ORC_SDK_REQUIRE((1 << DEPTH) == orc_sdk_deck_len(deck));
  ORC_SDK_REQUIRE(_orc_sdk_deck_header(deck)->item_size == sizeof(size_t));
  {  // Iterate from level 5.
    size_t          counter = 0;
    OrcSdk_DeckView v5      = orc_sdk_dv_from_deck(deck, 5);
    do {
      ORC_SDK_REQUIRE(5 == orc_sdk_dv_depth(&v5));
      ORC_SDK_REQUIRE(32 == orc_sdk_dv_len(&v5));
      OrcSdk_DeckView v4 = orc_sdk_dv_child(&v5);
      do {
        ORC_SDK_REQUIRE(4 == orc_sdk_dv_depth(&v4));
        ORC_SDK_REQUIRE(16 == orc_sdk_dv_len(&v4));
        OrcSdk_DeckView v3 = orc_sdk_dv_child(&v4);
        do {
          ORC_SDK_REQUIRE(3 == orc_sdk_dv_depth(&v3));
          ORC_SDK_REQUIRE(8 == orc_sdk_dv_len(&v3));
          OrcSdk_DeckView v2 = orc_sdk_dv_child(&v3);
          do {
            ORC_SDK_REQUIRE(2 == orc_sdk_dv_depth(&v2));
            ORC_SDK_REQUIRE(4 == orc_sdk_dv_len(&v2));
            OrcSdk_DeckView v1 = orc_sdk_dv_child(&v2);
            do {
              ORC_SDK_REQUIRE(1 == orc_sdk_dv_depth(&v1));
              ORC_SDK_REQUIRE(2 == orc_sdk_dv_len(&v1));
              size_t const *items = orc_sdk_dv_item_ptr(&v1);
              ORC_SDK_REQUIRE(items != NULL);
              size_t const *end = items + orc_sdk_dv_len(&v1);
              while (items != end) {
                ORC_SDK_REQUIRE(*(items++) == counter++);
              }
            } while (orc_sdk_dv_advance(&v1));
          } while (orc_sdk_dv_advance(&v2));
        } while (orc_sdk_dv_advance(&v3));
      } while (orc_sdk_dv_advance(&v4));
    } while (orc_sdk_dv_advance(&v5));
    ORC_SDK_REQUIRE(counter == 32);
  }
  {  // Iterate from level 4.
    OrcSdk_DeckView v4      = orc_sdk_dv_from_deck(deck, 4);
    size_t          counter = 0;
    do {
      ORC_SDK_REQUIRE(4 == orc_sdk_dv_depth(&v4));
      ORC_SDK_REQUIRE(16 == orc_sdk_dv_len(&v4));
      OrcSdk_DeckView v3 = orc_sdk_dv_child(&v4);
      do {
        ORC_SDK_REQUIRE(3 == orc_sdk_dv_depth(&v3));
        ORC_SDK_REQUIRE(8 == orc_sdk_dv_len(&v3));
        OrcSdk_DeckView v2 = orc_sdk_dv_child(&v3);
        do {
          ORC_SDK_REQUIRE(2 == orc_sdk_dv_depth(&v2));
          ORC_SDK_REQUIRE(4 == orc_sdk_dv_len(&v2));
          OrcSdk_DeckView v1 = orc_sdk_dv_child(&v2);
          do {
            ORC_SDK_REQUIRE(1 == orc_sdk_dv_depth(&v1));
            ORC_SDK_REQUIRE(2 == orc_sdk_dv_len(&v1));
            size_t const *items = orc_sdk_dv_item_ptr(&v1);
            ORC_SDK_REQUIRE(items != NULL);
            size_t const *end = items + orc_sdk_dv_len(&v1);
            while (items != end) {
              ORC_SDK_REQUIRE(*(items++) == counter++);
            }
          } while (orc_sdk_dv_advance(&v1));
        } while (orc_sdk_dv_advance(&v2));
      } while (orc_sdk_dv_advance(&v3));
    } while (orc_sdk_dv_advance(&v4));
    ORC_SDK_REQUIRE(counter == 32);
  }
  {  // Iterate from level 3.
    OrcSdk_DeckView v3      = orc_sdk_dv_from_deck(deck, 3);
    size_t          counter = 0;
    do {
      ORC_SDK_REQUIRE(3 == orc_sdk_dv_depth(&v3));
      ORC_SDK_REQUIRE(8 == orc_sdk_dv_len(&v3));
      OrcSdk_DeckView v2 = orc_sdk_dv_child(&v3);
      do {
        ORC_SDK_REQUIRE(2 == orc_sdk_dv_depth(&v2));
        ORC_SDK_REQUIRE(4 == orc_sdk_dv_len(&v2));
        OrcSdk_DeckView v1 = orc_sdk_dv_child(&v2);
        do {
          ORC_SDK_REQUIRE(1 == orc_sdk_dv_depth(&v1));
          ORC_SDK_REQUIRE(2 == orc_sdk_dv_len(&v1));
          OrcSdk_DeckView v0 = orc_sdk_dv_child(&v1);
          do {
            size_t const *item = orc_sdk_dv_item_ptr(&v0);
            ORC_SDK_REQUIRE(item != NULL);
            ORC_SDK_REQUIRE(*item == counter++);
          } while (orc_sdk_dv_advance(&v0));
        } while (orc_sdk_dv_advance(&v1));
      } while (orc_sdk_dv_advance(&v2));
    } while (orc_sdk_dv_advance(&v3));
    ORC_SDK_REQUIRE(counter == 32);
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
        ORC_SDK_REQUIRE(orc_sdk_dw_push(&c, counter) == OK);
        counter++;
      }
      ORC_SDK_REQUIRE(orc_sdk_dw_len(&c) == 3);
    }
  }
  ORC_SDK_REQUIRE(orc_sdk_deck_len(deck) == 9);
  ORC_SDK_REQUIRE(orc_sdk_deck_max_depth(deck) == 2);
  ORC_SDK_REQUIRE(_orc_sdk_deck_header(deck)->item_size == sizeof(uint32_t));
  // Read back via OrcSdk_DeckView.
  counter            = 0;
  OrcSdk_DeckView v2 = orc_sdk_dv_from_deck(deck, 2);
  do {
    OrcSdk_DeckView v1 = orc_sdk_dv_child(&v2);
    do {
      uint32_t const *items = orc_sdk_dv_item_ptr(&v1);
      for (size_t i = 0; i < orc_sdk_dv_len(&v1); ++i) {
        ORC_SDK_REQUIRE(items[i] == counter++);
      }
    } while (orc_sdk_dv_advance(&v1));
  } while (orc_sdk_dv_advance(&v2));
  ORC_SDK_REQUIRE(counter == 9);
  orc_sdk_deck_free(deck);
}

void test_dw_depth3_nested(void)
{
  // Build 3x3x3 tree at depth 3, mirroring Rust t_deck_writer_basic.
  uint32_t *deck    = NULL;
  uint32_t  counter = 0;
  {
    OrcSdk_DeckWriter w3 = orc_sdk_dw_from_deck(deck, 3);
    ORC_SDK_REQUIRE(w3.depth == 3);
    for (int a = 0; a < 3; ++a) {
      OrcSdk_DeckWriter w2 = orc_sdk_dw_child(&w3);
      ORC_SDK_REQUIRE(w2.depth == 2);
      for (int b = 0; b < 3; ++b) {
        OrcSdk_DeckWriter w1 = orc_sdk_dw_child(&w2);
        ORC_SDK_REQUIRE(w1.depth == 1);
        for (int c = 0; c < 3; ++c) {
          ORC_SDK_REQUIRE(orc_sdk_dw_push(&w1, counter) == OK);
          counter++;
        }
        ORC_SDK_REQUIRE(orc_sdk_dw_len(&w1) == 3);
      }
    }
  }
  ORC_SDK_REQUIRE(orc_sdk_deck_len(deck) == 27);
  ORC_SDK_REQUIRE(orc_sdk_deck_max_depth(deck) == 3);
  ORC_SDK_REQUIRE(_orc_sdk_deck_header(deck)->item_size == sizeof(uint32_t));
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
          ORC_SDK_REQUIRE(items[i] == counter++);
        }
      } while (orc_sdk_dv_advance(&v1));
    } while (orc_sdk_dv_advance(&v2));
  } while (orc_sdk_dv_advance(&v3));
  ORC_SDK_REQUIRE(counter == 27);
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
      ORC_SDK_REQUIRE(orc_sdk_dw_push(&c, v) == OK);
    }
    {
      OrcSdk_DeckWriter c = orc_sdk_dw_child(&w);
      uint32_t          v;
      v = 2;
      ORC_SDK_REQUIRE(orc_sdk_dw_push(&c, v) == OK);
      v = 3;
      ORC_SDK_REQUIRE(orc_sdk_dw_push(&c, v) == OK);
      v = 4;
      ORC_SDK_REQUIRE(orc_sdk_dw_push(&c, v) == OK);
    }
    {
      OrcSdk_DeckWriter c = orc_sdk_dw_child(&w);
      uint32_t          v;
      v = 5;
      ORC_SDK_REQUIRE(orc_sdk_dw_push(&c, v) == OK);
      v = 6;
      ORC_SDK_REQUIRE(orc_sdk_dw_push(&c, v) == OK);
    }
  }
  ORC_SDK_REQUIRE(orc_sdk_deck_len(deck) == 6);
  ORC_SDK_REQUIRE(_orc_sdk_deck_header(deck)->item_size == sizeof(uint32_t));
  // Verify structure.
  OrcSdk_DeckView outer = orc_sdk_dv_from_deck(deck, 2);
  OrcSdk_DeckView g1    = orc_sdk_dv_child(&outer);
  ORC_SDK_REQUIRE(orc_sdk_dv_len(&g1) == 1);
  ORC_SDK_REQUIRE(*(uint32_t *)orc_sdk_dv_item_ptr(&g1) == 1);
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
        ORC_SDK_REQUIRE(items[i] == expected[idx++]);
      }
    } while (orc_sdk_dv_advance(&inner));
  } while (orc_sdk_dv_advance(&top));
  ORC_SDK_REQUIRE(idx == 6);
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
      ORC_SDK_REQUIRE(orc_sdk_dw_push(&c, v) == OK);
      v = 2;
      ORC_SDK_REQUIRE(orc_sdk_dw_push(&c, v) == OK);
    }
    {
      OrcSdk_DeckWriter c = orc_sdk_dw_child(&w);
      orc_sdk_dw_close(&c);  // empty group
    }
    {
      OrcSdk_DeckWriter c = orc_sdk_dw_child(&w);
      uint32_t          v = 3;
      ORC_SDK_REQUIRE(orc_sdk_dw_push(&c, v) == OK);
    }
    {
      OrcSdk_DeckWriter c = orc_sdk_dw_child(&w);
      orc_sdk_dw_close(&c);  // empty group
    }
  }
  ORC_SDK_REQUIRE(orc_sdk_deck_len(deck) == 3);
  ORC_SDK_REQUIRE(_orc_sdk_deck_header(deck)->item_size == sizeof(uint32_t));
  ORC_SDK_REQUIRE(deck[0] == 1);
  ORC_SDK_REQUIRE(deck[1] == 2);
  ORC_SDK_REQUIRE(deck[2] == 3);
  // Verify: 5 inner groups, sizes 0,2,0,1,0.
  size_t          group_sizes[] = {0, 2, 0, 1, 0};
  size_t          gi            = 0;
  OrcSdk_DeckView top           = orc_sdk_dv_from_deck(deck, 2);
  do {
    OrcSdk_DeckView inner = orc_sdk_dv_child(&top);
    do {
      ORC_SDK_REQUIRE(orc_sdk_dv_len(&inner) == group_sizes[gi++]);
    } while (orc_sdk_dv_advance(&inner));
  } while (orc_sdk_dv_advance(&top));
  ORC_SDK_REQUIRE(gi == 5);
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
        ORC_SDK_REQUIRE(orc_sdk_dw_push(&w1, v) == OK);
        v = 2;
        ORC_SDK_REQUIRE(orc_sdk_dw_push(&w1, v) == OK);
      }
    }
    {
      OrcSdk_DeckWriter w2 = orc_sdk_dw_child(&w3);
      {
        OrcSdk_DeckWriter w1 = orc_sdk_dw_child(&w2);
        uint32_t          v  = 3;
        ORC_SDK_REQUIRE(orc_sdk_dw_push(&w1, v) == OK);
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
  ORC_SDK_REQUIRE(orc_sdk_deck_len(deck) == 3);
  ORC_SDK_REQUIRE(orc_sdk_deck_max_depth(deck) == 3);
  ORC_SDK_REQUIRE(_orc_sdk_deck_header(deck)->item_size == sizeof(uint32_t));
  // Verify structure by iterating depth 3 → 2 → 1.
  // Expected: (((), (1,2)), ((3,)), (()))
  OrcSdk_DeckView v3 = orc_sdk_dv_from_deck(deck, 3);
  // Only one top-level group.
  OrcSdk_DeckView mid = orc_sdk_dv_child(&v3);
  // First mid group: ((), (1,2))
  {
    OrcSdk_DeckView inner = orc_sdk_dv_child(&mid);
    ORC_SDK_REQUIRE(orc_sdk_dv_len(&inner) == 0);  // empty
    ORC_SDK_REQUIRE(orc_sdk_dv_advance(&inner));
    ORC_SDK_REQUIRE(orc_sdk_dv_len(&inner) == 2);
    uint32_t const *items = orc_sdk_dv_item_ptr(&inner);
    ORC_SDK_REQUIRE(items[0] == 1);
    ORC_SDK_REQUIRE(items[1] == 2);
    ORC_SDK_REQUIRE(!orc_sdk_dv_advance(&inner));
  }
  ORC_SDK_REQUIRE(orc_sdk_dv_advance(&mid));
  // Second mid group: ((3,))
  {
    OrcSdk_DeckView inner = orc_sdk_dv_child(&mid);
    ORC_SDK_REQUIRE(orc_sdk_dv_len(&inner) == 1);
    ORC_SDK_REQUIRE(*(uint32_t *)orc_sdk_dv_item_ptr(&inner) == 3);
    ORC_SDK_REQUIRE(!orc_sdk_dv_advance(&inner));
  }
  ORC_SDK_REQUIRE(orc_sdk_dv_advance(&mid));
  // Third mid group: (())
  {
    OrcSdk_DeckView inner = orc_sdk_dv_child(&mid);
    ORC_SDK_REQUIRE(orc_sdk_dv_len(&inner) == 0);
    ORC_SDK_REQUIRE(!orc_sdk_dv_advance(&inner));
  }
  ORC_SDK_REQUIRE(!orc_sdk_dv_advance(&mid));
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
    ORC_SDK_REQUIRE(orc_sdk_dw_push(&w1, v) == OK);
  }
  ORC_SDK_REQUIRE(orc_sdk_deck_len(deck) == 1);
  ORC_SDK_REQUIRE(orc_sdk_deck_max_depth(deck) == 5);
  ORC_SDK_REQUIRE(_orc_sdk_deck_header(deck)->item_size == sizeof(uint32_t));
  ORC_SDK_REQUIRE(deck[0] == 42);
  // Unwrap all the way down.
  OrcSdk_DeckView v5 = orc_sdk_dv_from_deck(deck, 5);
  OrcSdk_DeckView v4 = orc_sdk_dv_child(&v5);
  OrcSdk_DeckView v3 = orc_sdk_dv_child(&v4);
  OrcSdk_DeckView v2 = orc_sdk_dv_child(&v3);
  OrcSdk_DeckView v1 = orc_sdk_dv_child(&v2);
  ORC_SDK_REQUIRE(orc_sdk_dv_len(&v1) == 1);
  ORC_SDK_REQUIRE(*(uint32_t *)orc_sdk_dv_item_ptr(&v1) == 42);
  orc_sdk_deck_free(deck);
}

void test_orc_sdk_dw_len_tracking(void)
{
  // Verify orc_sdk_dw_len reflects items added at each scope level.
  uint32_t *deck = NULL;
  {
    OrcSdk_DeckWriter w3 = orc_sdk_dw_from_deck(deck, 3);
    ORC_SDK_REQUIRE(orc_sdk_dw_len(&w3) == 0);
    {
      OrcSdk_DeckWriter w2 = orc_sdk_dw_child(&w3);
      ORC_SDK_REQUIRE(orc_sdk_dw_len(&w2) == 0);
      {
        OrcSdk_DeckWriter w1 = orc_sdk_dw_child(&w2);
        ORC_SDK_REQUIRE(orc_sdk_dw_len(&w1) == 0);
        uint32_t v = 10;
        orc_sdk_dw_push(&w1, v);
        ORC_SDK_REQUIRE(orc_sdk_dw_len(&w1) == 1);
        v = 20;
        orc_sdk_dw_push(&w1, v);
        ORC_SDK_REQUIRE(orc_sdk_dw_len(&w1) == 2);
      }
      ORC_SDK_REQUIRE(orc_sdk_dw_len(&w2) == 2);
      {
        OrcSdk_DeckWriter w1 = orc_sdk_dw_child(&w2);
        uint32_t          v  = 30;
        orc_sdk_dw_push(&w1, v);
        ORC_SDK_REQUIRE(orc_sdk_dw_len(&w1) == 1);
      }
      ORC_SDK_REQUIRE(orc_sdk_dw_len(&w2) == 3);
    }
    ORC_SDK_REQUIRE(orc_sdk_dw_len(&w3) == 3);
    {
      OrcSdk_DeckWriter w2 = orc_sdk_dw_child(&w3);
      ORC_SDK_REQUIRE(orc_sdk_dw_len(&w2) == 0);
      {
        OrcSdk_DeckWriter w1 = orc_sdk_dw_child(&w2);
        uint32_t          v  = 40;
        orc_sdk_dw_push(&w1, v);
      }
      ORC_SDK_REQUIRE(orc_sdk_dw_len(&w2) == 1);
    }
    ORC_SDK_REQUIRE(orc_sdk_dw_len(&w3) == 4);
  }
  ORC_SDK_REQUIRE(orc_sdk_deck_len(deck) == 4);
  ORC_SDK_REQUIRE(_orc_sdk_deck_header(deck)->item_size == sizeof(uint32_t));
  orc_sdk_deck_free(deck);
}

void test_dw_append_to_existing(void)
{
  // Build ((1,2),(3,4)) manually, then append (5,6) via writer.
  uint32_t *deck = NULL;
  uint32_t  v;
  v = 1;
  ORC_SDK_REQUIRE(OK == orc_sdk_deck_push(deck, v, 2));
  v = 2;
  ORC_SDK_REQUIRE(OK == orc_sdk_deck_push(deck, v, 0));
  v = 3;
  ORC_SDK_REQUIRE(OK == orc_sdk_deck_push(deck, v, 1));
  v = 4;
  ORC_SDK_REQUIRE(OK == orc_sdk_deck_push(deck, v, 0));
  ORC_SDK_REQUIRE(orc_sdk_deck_len(deck) == 4);
  ORC_SDK_REQUIRE(_orc_sdk_deck_header(deck)->item_size == sizeof(uint32_t));
  // Append another depth-1 group.
  {
    OrcSdk_DeckWriter w = orc_sdk_dw_from_deck(deck, 1);
    v                   = 5;
    orc_sdk_dw_push(&w, v);
    v = 6;
    orc_sdk_dw_push(&w, v);
  }
  ORC_SDK_REQUIRE(orc_sdk_deck_len(deck) == 6);
  ORC_SDK_REQUIRE(_orc_sdk_deck_header(deck)->item_size == sizeof(uint32_t));
  // Verify: ((1,2),(3,4),(5,6))
  uint32_t        counter = 0;
  OrcSdk_DeckView top     = orc_sdk_dv_from_deck(deck, 2);
  do {
    OrcSdk_DeckView inner = orc_sdk_dv_child(&top);
    size_t          n     = 0;
    do {
      uint32_t const *items = orc_sdk_dv_item_ptr(&inner);
      for (size_t i = 0; i < orc_sdk_dv_len(&inner); ++i) {
        ORC_SDK_REQUIRE(items[i] == ++counter);
      }
      n++;
    } while (orc_sdk_dv_advance(&inner));
    ORC_SDK_REQUIRE(n * 2 <= 6);  // each group has 2 items
  } while (orc_sdk_dv_advance(&top));
  ORC_SDK_REQUIRE(counter == 6);
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
    ORC_SDK_REQUIRE(orc_sdk_dw_len(&w) == 5);
  }
  ORC_SDK_REQUIRE(orc_sdk_deck_len(deck) == 5);
  ORC_SDK_REQUIRE(orc_sdk_deck_max_depth(deck) == 1);
  ORC_SDK_REQUIRE(_orc_sdk_deck_header(deck)->item_size == sizeof(uint32_t));
  for (uint32_t i = 0; i < 5; ++i) {
    ORC_SDK_REQUIRE(deck[i] == i * 10);
  }
  // Verify via view: one group with 5 items.
  OrcSdk_DeckView v1 = orc_sdk_dv_from_deck(deck, 1);
  ORC_SDK_REQUIRE(orc_sdk_dv_len(&v1) == 5);
  ORC_SDK_REQUIRE(!orc_sdk_dv_advance(&v1));
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
      ORC_SDK_REQUIRE(items[0] == 100);
      ORC_SDK_REQUIRE(items[1] == 200);
      ORC_SDK_REQUIRE(items[2] == 300);
      ORC_SDK_REQUIRE(orc_sdk_dw_len(&c) == 3);
    }
    {
      OrcSdk_DeckWriter c = orc_sdk_dw_child(&w);
      uint32_t          v;
      v = 400;
      orc_sdk_dw_push(&c, v);
      v = 500;
      orc_sdk_dw_push(&c, v);
      uint32_t *items = orc_sdk_deck_item_ptr(&c);
      ORC_SDK_REQUIRE(items[0] == 400);
      ORC_SDK_REQUIRE(items[1] == 500);
      ORC_SDK_REQUIRE(orc_sdk_dw_len(&c) == 2);
    }
  }
  ORC_SDK_REQUIRE(orc_sdk_deck_len(deck) == 5);
  ORC_SDK_REQUIRE(_orc_sdk_deck_header(deck)->item_size == sizeof(uint32_t));
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
  ORC_SDK_REQUIRE(orc_sdk_deck_len(deck) == 2);
  ORC_SDK_REQUIRE(_orc_sdk_deck_header(deck)->item_size == sizeof(uint32_t));
  // Verify: ((1), (), (2))
  size_t          group_sizes[] = {1, 0, 1};
  size_t          gi            = 0;
  OrcSdk_DeckView top           = orc_sdk_dv_from_deck(deck, 2);
  do {
    OrcSdk_DeckView inner = orc_sdk_dv_child(&top);
    do {
      ORC_SDK_REQUIRE(orc_sdk_dv_len(&inner) == group_sizes[gi++]);
    } while (orc_sdk_dv_advance(&inner));
  } while (orc_sdk_dv_advance(&top));
  ORC_SDK_REQUIRE(gi == 3);
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
  ORC_SDK_REQUIRE(orc_sdk_deck_len(deck) == 0);
  ORC_SDK_REQUIRE(orc_sdk_deck_max_depth(deck) == 3);
  ORC_SDK_REQUIRE(_orc_sdk_deck_header(deck)->item_size == sizeof(uint32_t));
  // Verify: one top group, two mid groups, inner groups all empty.
  OrcSdk_DeckView v3  = orc_sdk_dv_from_deck(deck, 3);
  OrcSdk_DeckView mid = orc_sdk_dv_child(&v3);
  // First mid: 2 empty children.
  {
    OrcSdk_DeckView inner = orc_sdk_dv_child(&mid);
    ORC_SDK_REQUIRE(orc_sdk_dv_len(&inner) == 0);
    ORC_SDK_REQUIRE(orc_sdk_dv_advance(&inner));
    ORC_SDK_REQUIRE(orc_sdk_dv_len(&inner) == 0);
    ORC_SDK_REQUIRE(!orc_sdk_dv_advance(&inner));
  }
  ORC_SDK_REQUIRE(orc_sdk_dv_advance(&mid));
  // Second mid: 1 empty child.
  {
    OrcSdk_DeckView inner = orc_sdk_dv_child(&mid);
    ORC_SDK_REQUIRE(orc_sdk_dv_len(&inner) == 0);
    ORC_SDK_REQUIRE(!orc_sdk_dv_advance(&inner));
  }
  ORC_SDK_REQUIRE(!orc_sdk_dv_advance(&mid));
  orc_sdk_deck_free(deck);
}

// ========== Dims (Units) ==========

void test_orc_sdk_dims_equal(void)
{
  OrcDims a = {1, 0, -2, 0, 0, 0, 0};
  OrcDims b = {1, 0, -2, 0, 0, 0, 0};
  OrcDims c = {1, 0, -1, 0, 0, 0, 0};
  ORC_SDK_REQUIRE(orc_sdk_dims_equal(a, b));
  ORC_SDK_REQUIRE(!orc_sdk_dims_equal(a, c));
  // Dimensionless
  OrcDims zero_a = {0, 0, 0, 0, 0, 0, 0};
  OrcDims zero_b = {0, 0, 0, 0, 0, 0, 0};
  ORC_SDK_REQUIRE(orc_sdk_dims_equal(zero_a, zero_b));
  // Differ only in last dimension
  OrcDims d = {0, 0, 0, 0, 0, 0, 1};
  ORC_SDK_REQUIRE(!orc_sdk_dims_equal(zero_a, d));
}

void test_orc_sdk_dims_multiply(void)
{
  // force * distance = energy
  OrcDims force  = {1, 1, -2, 0, 0, 0, 0};
  OrcDims length = {1, 0, 0, 0, 0, 0, 0};
  OrcDims out;
  orc_sdk_dims_multiply(force, length, out);
  OrcDims energy = {2, 1, -2, 0, 0, 0, 0};
  ORC_SDK_REQUIRE(orc_sdk_dims_equal(out, energy));
  // Multiply by dimensionless is identity
  OrcDims zero = {0, 0, 0, 0, 0, 0, 0};
  orc_sdk_dims_multiply(force, zero, out);
  ORC_SDK_REQUIRE(orc_sdk_dims_equal(out, force));
  // Negative exponents cancel
  OrcDims a = {-1, -1, 3, 0, 0, 0, 0};
  OrcDims b = {1, 1, -3, 0, 0, 0, 0};
  orc_sdk_dims_multiply(a, b, out);
  ORC_SDK_REQUIRE(orc_sdk_dims_equal(out, zero));
}

void test_orc_sdk_dims_divide(void)
{
  // velocity / time = acceleration
  OrcDims velocity = {1, 0, -1, 0, 0, 0, 0};
  OrcDims time     = {0, 0, 1, 0, 0, 0, 0};
  OrcDims out;
  orc_sdk_dims_divide(velocity, time, out);
  OrcDims accel = {1, 0, -2, 0, 0, 0, 0};
  ORC_SDK_REQUIRE(orc_sdk_dims_equal(out, accel));
  // Divide by self = dimensionless
  orc_sdk_dims_divide(velocity, velocity, out);
  OrcDims zero = {0, 0, 0, 0, 0, 0, 0};
  ORC_SDK_REQUIRE(orc_sdk_dims_equal(out, zero));
  // Divide dimensionless by something = negated exponents
  orc_sdk_dims_divide(zero, time, out);
  OrcDims inv_time = {0, 0, -1, 0, 0, 0, 0};
  ORC_SDK_REQUIRE(orc_sdk_dims_equal(out, inv_time));
}

void test_orc_sdk_dims_pow(void)
{
  OrcDims length = {1, 0, 0, 0, 0, 0, 0};
  OrcDims out;
  // length^2 = area
  orc_sdk_dims_pow(length, 2, out);
  OrcDims area = {2, 0, 0, 0, 0, 0, 0};
  ORC_SDK_REQUIRE(orc_sdk_dims_equal(out, area));
  // length^3 = volume
  orc_sdk_dims_pow(length, 3, out);
  OrcDims volume = {3, 0, 0, 0, 0, 0, 0};
  ORC_SDK_REQUIRE(orc_sdk_dims_equal(out, volume));
  // pow 0 = dimensionless
  OrcDims velocity = {1, 0, -1, 0, 0, 0, 0};
  orc_sdk_dims_pow(velocity, 0, out);
  OrcDims zero = {0, 0, 0, 0, 0, 0, 0};
  ORC_SDK_REQUIRE(orc_sdk_dims_equal(out, zero));
  // pow 1 = identity
  orc_sdk_dims_pow(velocity, 1, out);
  ORC_SDK_REQUIRE(orc_sdk_dims_equal(out, velocity));
  // Negative power
  orc_sdk_dims_pow(velocity, -1, out);
  OrcDims inv_vel = {-1, 0, 1, 0, 0, 0, 0};
  ORC_SDK_REQUIRE(orc_sdk_dims_equal(out, inv_vel));
  // pow -2 on multi-dim
  OrcDims force = {1, 1, -2, 0, 0, 0, 0};
  orc_sdk_dims_pow(force, -2, out);
  OrcDims expected = {-2, -2, 4, 0, 0, 0, 0};
  ORC_SDK_REQUIRE(orc_sdk_dims_equal(out, expected));
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
  ORC_SDK_REQUIRE(list_handle->type_id == ORC_TYPE_F64);
  ORC_SDK_REQUIRE(index_handle->type_id == ORC_TYPE_U32);
  ORC_SDK_REQUIRE(item_handle->type_id == ORC_TYPE_F64);
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
    ORC_SDK_REQUIRE(list_input.depth == 1);
    ORC_SDK_REQUIRE(index_input.depth == 0);
    // Get output for the current combination.
    OrcSdk_DeckWriter *item_ouput = orc_sdk_comb_get_output(combinations, 0);
    ORC_SDK_REQUIRE(item_ouput->depth == 0);
    double *output_ptr = (double *)orc_sdk_dw_push_empty(item_ouput);
    {  // This scope simulates the actual doRun of the block.
      double        *list  = (double *)orc_sdk_dv_item_ptr(&list_input);
      uint32_t const index = *(uint32_t *)orc_sdk_dv_item_ptr(&index_input);
      ORC_SDK_REQUIRE_WITH_MSG(index < orc_sdk_dv_len(&list_input),
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
  ORC_SDK_REQUIRE(a_handle->type_id == ORC_TYPE_F64);
  ORC_SDK_REQUIRE(b_handle->type_id == ORC_TYPE_F64);
  ORC_SDK_REQUIRE(out_handle->type_id == ORC_TYPE_F64);
  void *combinations = orc_sdk_comb_init((OrcHandle const *[]) {a_handle, b_handle},
                                         (uint8_t const[]) {0, 0},
                                         2,
                                         (OrcHandle *[]) {out_handle},
                                         (uint8_t const[]) {0},
                                         1);
  while (combinations) {
    OrcSdk_DeckView a_input = orc_sdk_comb_get_input(combinations, 0),
                    b_input = orc_sdk_comb_get_input(combinations, 1);
    ORC_SDK_REQUIRE(a_input.depth == 0);
    ORC_SDK_REQUIRE(b_input.depth == 0);
    OrcSdk_DeckWriter *out = orc_sdk_comb_get_output(combinations, 0);
    ORC_SDK_REQUIRE(out->depth == 0);
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
  ORC_SDK_REQUIRE(in_handle->type_id == ORC_TYPE_F64);
  ORC_SDK_REQUIRE(out_sq_handle->type_id == ORC_TYPE_F64);
  ORC_SDK_REQUIRE(out_cb_handle->type_id == ORC_TYPE_F64);
  void *combinations = orc_sdk_comb_init((OrcHandle const *[]) {in_handle},
                                         (uint8_t const[]) {0},
                                         1,
                                         (OrcHandle *[]) {out_sq_handle, out_cb_handle},
                                         (uint8_t const[]) {0, 0},
                                         2);
  while (combinations) {
    OrcSdk_DeckView in_input = orc_sdk_comb_get_input(combinations, 0);
    ORC_SDK_REQUIRE(in_input.depth == 0);
    OrcSdk_DeckWriter *out_sq = orc_sdk_comb_get_output(combinations, 0);
    OrcSdk_DeckWriter *out_cb = orc_sdk_comb_get_output(combinations, 1);
    ORC_SDK_REQUIRE(out_sq->depth == 0);
    ORC_SDK_REQUIRE(out_cb->depth == 0);
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
  ORC_SDK_REQUIRE(a_handle->type_id == ORC_TYPE_F64);
  ORC_SDK_REQUIRE(b_handle->type_id == ORC_TYPE_F64);
  ORC_SDK_REQUIRE(out_sum_handle->type_id == ORC_TYPE_F64);
  ORC_SDK_REQUIRE(out_prod_handle->type_id == ORC_TYPE_F64);
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
    ORC_SDK_REQUIRE(a_input.depth == 0);
    ORC_SDK_REQUIRE(b_input.depth == 0);
    OrcSdk_DeckWriter *out_sum  = orc_sdk_comb_get_output(combinations, 0);
    OrcSdk_DeckWriter *out_prod = orc_sdk_comb_get_output(combinations, 1);
    ORC_SDK_REQUIRE(out_sum->depth == 0);
    ORC_SDK_REQUIRE(out_prod->depth == 0);
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
  ORC_SDK_REQUIRE(a_handle->type_id == ORC_TYPE_F64);
  ORC_SDK_REQUIRE(b_handle->type_id == ORC_TYPE_F64);
  ORC_SDK_REQUIRE(out_handle->type_id == ORC_TYPE_F64);
  void *combinations = orc_sdk_comb_init((OrcHandle const *[]) {a_handle, b_handle},
                                         (uint8_t const[]) {1, 1},
                                         2,
                                         (OrcHandle *[]) {out_handle},
                                         (uint8_t const[]) {0},
                                         1);
  while (combinations) {
    OrcSdk_DeckView a_input = orc_sdk_comb_get_input(combinations, 0),
                    b_input = orc_sdk_comb_get_input(combinations, 1);
    ORC_SDK_REQUIRE(a_input.depth == 1);
    ORC_SDK_REQUIRE(b_input.depth == 1);
    OrcSdk_DeckWriter *out = orc_sdk_comb_get_output(combinations, 0);
    ORC_SDK_REQUIRE(out->depth == 0);
    double *output_ptr = (double *)orc_sdk_dw_push_empty(out);
    {
      ORC_SDK_REQUIRE_WITH_MSG(
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
  orc_sdk_handle_alloc(ORC_TYPE_F64, &lists);
  orc_sdk_handle_alloc(ORC_TYPE_U32, &indices);
  orc_sdk_handle_alloc(ORC_TYPE_F64, &out_items);
  ORC_SDK_REQUIRE_WITH_MSG(
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
    ORC_SDK_REQUIRE(orc_sdk_deck_len(lists.items) == 18);
    ORC_SDK_DECK_INIT(indices.items, uint32_t, 2);
    orc_sdk_oh_update(&indices);
    ORC_SDK_REQUIRE(orc_sdk_deck_len(indices.items) == 1);
    // Run the block - In a real scenario, this function is provided by a plugin DLL.
    _plugin_function_list_element(&lists, &indices, &out_items);
    // Check the outputs. The input had 4 lists, so the output should have 4 items.
    orc_sdk_oh_update(&out_items);
    size_t const count = orc_sdk_deck_len(out_items.items);
    ORC_SDK_REQUIRE(count == 4);
    ORC_SDK_REQUIRE(orc_sdk_deck_max_depth(out_items.items) == 1);
    // Output should contain the #2 item from every input list.
    double const  expected[] = {3.34, 6.6, 9.9, 13.1};
    double *const actual     = (double *)out_items.items;
    for (size_t i = 0; i < count; ++i) {
      ORC_SDK_REQUIRE(actual[i] == expected[i]);
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
    ORC_SDK_REQUIRE(count == 4);
    ORC_SDK_REQUIRE(orc_sdk_deck_max_depth(out_items.items) == 1);
    double const  expected[] = {1.1, 5.5, 9.9, 13.1};
    double *const actual     = (double *)out_items.items;
    for (size_t i = 0; i < count; ++i) {
      ORC_SDK_REQUIRE(actual[i] == expected[i]);
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
    ORC_SDK_REQUIRE(orc_sdk_deck_max_depth(actual) == orc_sdk_deck_max_depth(expected));
    size_t n_marks = 0;
    {
      _OrcSdk_DeckHeader *h = _orc_sdk_deck_header(actual);
      n_marks               = orc_sdk_arr_len(h->marks);
      ORC_SDK_REQUIRE(orc_sdk_arr_len(_orc_sdk_deck_header(expected)->marks) == n_marks);
    }
    for (size_t i = 0; i < n_marks; ++i) {
      OrcMark const m1 = _orc_sdk_deck_header(expected)->marks[i];
      OrcMark const m2 = _orc_sdk_deck_header(actual)->marks[i];
      ORC_SDK_REQUIRE(m1.pos == m2.pos && m1.depth == m2.depth);
    }
    size_t const count = orc_sdk_deck_len(actual);
    ORC_SDK_REQUIRE(count == orc_sdk_deck_len(expected));
    for (size_t i = 0; i < count; ++i) {
      ORC_SDK_REQUIRE(actual[i] == expected[i]);
    }
    orc_sdk_deck_free(expected);
  }
  // Clean up decks - In a real scenario, the host program is cleaning up, by calling
  // below functions, which are defined inside a plugin.
  ORC_SDK_REQUIRE(ORC_ERROR_NONE == orc_sdk_handle_free(&lists));
  ORC_SDK_REQUIRE(ORC_ERROR_NONE == orc_sdk_handle_free(&indices));
  ORC_SDK_REQUIRE(ORC_ERROR_NONE == orc_sdk_handle_free(&out_items));
}

void test_add_f64_combinations(void)
{
  /*=== Tests two-input scalar addition: equal lengths, broadcast, and depth-2 inputs.
   * ===*/
  OrcHandle a = {0}, b = {0}, out = {0};
  orc_sdk_handle_alloc(ORC_TYPE_F64, &a);
  orc_sdk_handle_alloc(ORC_TYPE_F64, &b);
  orc_sdk_handle_alloc(ORC_TYPE_F64, &out);
  ORC_SDK_REQUIRE_WITH_MSG(a.items != NULL && b.items != NULL && out.items != NULL,
                           "Unable to allocate decks");

  { /* Flat equal-length inputs: a and b each have 3 scalars (stack_depth=2). */
    ORC_SDK_DECK_INIT(a.items, double, (1.0, 2.0, 3.0));
    orc_sdk_oh_update(&a);
    ORC_SDK_DECK_INIT(b.items, double, (10.0, 20.0, 30.0));
    orc_sdk_oh_update(&b);

    _plugin_function_add_f64(&a, &b, &out);

    orc_sdk_oh_update(&out);
    size_t const count = orc_sdk_deck_len(out.items);
    ORC_SDK_REQUIRE(count == 3);
    ORC_SDK_REQUIRE(orc_sdk_deck_max_depth(out.items) == 1);
    double const  expected[] = {11.0, 22.0, 33.0};
    double *const actual     = (double *)out.items;
    for (size_t i = 0; i < count; ++i) {
      ORC_SDK_REQUIRE(actual[i] == expected[i]);
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
    ORC_SDK_REQUIRE(count == 4);
    ORC_SDK_REQUIRE(orc_sdk_deck_max_depth(out.items) == 1);
    // b is exhausted at 20.0 and stays there for the remaining elements of a.
    double const  expected[] = {11.0, 22.0, 23.0, 24.0};
    double *const actual     = (double *)out.items;
    for (size_t i = 0; i < count; ++i) {
      ORC_SDK_REQUIRE(actual[i] == expected[i]);
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
    ORC_SDK_REQUIRE(orc_sdk_deck_max_depth(actual) == orc_sdk_deck_max_depth(expected));
    size_t n_marks = 0;
    {
      _OrcSdk_DeckHeader *h = _orc_sdk_deck_header(actual);
      n_marks               = orc_sdk_arr_len(h->marks);
      ORC_SDK_REQUIRE(orc_sdk_arr_len(_orc_sdk_deck_header(expected)->marks) == n_marks);
    }
    for (size_t i = 0; i < n_marks; ++i) {
      OrcMark const m1 = _orc_sdk_deck_header(expected)->marks[i];
      OrcMark const m2 = _orc_sdk_deck_header(actual)->marks[i];
      ORC_SDK_REQUIRE(m1.pos == m2.pos && m1.depth == m2.depth);
    }
    size_t const count = orc_sdk_deck_len(actual);
    ORC_SDK_REQUIRE(count == orc_sdk_deck_len(expected));
    for (size_t i = 0; i < count; ++i) {
      ORC_SDK_REQUIRE(actual[i] == expected[i]);
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
    ORC_SDK_REQUIRE(orc_sdk_deck_max_depth(actual) == orc_sdk_deck_max_depth(expected));
    size_t n_marks = 0;
    {
      _OrcSdk_DeckHeader *h = _orc_sdk_deck_header(actual);
      n_marks               = orc_sdk_arr_len(h->marks);
      ORC_SDK_REQUIRE(orc_sdk_arr_len(_orc_sdk_deck_header(expected)->marks) == n_marks);
    }
    for (size_t i = 0; i < n_marks; ++i) {
      OrcMark const m1 = _orc_sdk_deck_header(expected)->marks[i];
      OrcMark const m2 = _orc_sdk_deck_header(actual)->marks[i];
      ORC_SDK_REQUIRE(m1.pos == m2.pos && m1.depth == m2.depth);
    }
    size_t const count = orc_sdk_deck_len(actual);
    ORC_SDK_REQUIRE(count == orc_sdk_deck_len(expected));
    for (size_t i = 0; i < count; ++i) {
      ORC_SDK_REQUIRE(actual[i] == expected[i]);
    }
    orc_sdk_deck_free(expected);
  }
  ORC_SDK_REQUIRE(ORC_ERROR_NONE == orc_sdk_handle_free(&a));
  ORC_SDK_REQUIRE(ORC_ERROR_NONE == orc_sdk_handle_free(&b));
  ORC_SDK_REQUIRE(ORC_ERROR_NONE == orc_sdk_handle_free(&out));
}

// This simulates a function that takes depth=1 lists of F64 and outputs the length of
// each list as U64.
void _plugin_function_list_length(OrcHandle const *in_handle, OrcHandle *out_handle)
{
  ORC_SDK_REQUIRE(in_handle->type_id == ORC_TYPE_F64);
  ORC_SDK_REQUIRE(out_handle->type_id == ORC_TYPE_U64);
  void *combinations = orc_sdk_comb_init((OrcHandle const *[]) {in_handle},
                                         (uint8_t const[]) {1},
                                         1,
                                         (OrcHandle *[]) {out_handle},
                                         (uint8_t const[]) {0},
                                         1);
  while (combinations) {
    OrcSdk_DeckView    list_input = orc_sdk_comb_get_input(combinations, 0);
    OrcSdk_DeckWriter *out        = orc_sdk_comb_get_output(combinations, 0);
    ORC_SDK_REQUIRE(list_input.depth == 1);
    ORC_SDK_REQUIRE(out->depth == 0);
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
  orc_sdk_handle_alloc(ORC_TYPE_F64, &in);
  orc_sdk_handle_alloc(ORC_TYPE_U64, &out);
  ORC_SDK_REQUIRE_WITH_MSG(in.items != NULL && out.items != NULL,
                           "Unable to allocate decks");

  { /* Depth-2 input: 5 lists, some empty (stack_depth=2). */
    ORC_SDK_DECK_INIT(in.items, double, ((1.0, 2.0, 3.0), (), (4.0), (), (5.0, 6.0)));
    orc_sdk_oh_update(&in);

    _plugin_function_list_length(&in, &out);

    orc_sdk_oh_update(&out);
    size_t const count = orc_sdk_deck_len(out.items);
    ORC_SDK_REQUIRE(count == 5);
    ORC_SDK_REQUIRE(orc_sdk_deck_max_depth(out.items) == 1);
    uint64_t const  expected[] = {3, 0, 1, 0, 2};
    uint64_t *const actual     = (uint64_t *)out.items;
    for (size_t i = 0; i < count; ++i) {
      ORC_SDK_REQUIRE(actual[i] == expected[i]);
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
    ORC_SDK_REQUIRE(orc_sdk_deck_max_depth(actual) == orc_sdk_deck_max_depth(expected));
    size_t n_marks = 0;
    {
      _OrcSdk_DeckHeader *h = _orc_sdk_deck_header(actual);
      n_marks               = orc_sdk_arr_len(h->marks);
      ORC_SDK_REQUIRE(orc_sdk_arr_len(_orc_sdk_deck_header(expected)->marks) == n_marks);
    }
    for (size_t i = 0; i < n_marks; ++i) {
      OrcMark const m1 = _orc_sdk_deck_header(expected)->marks[i];
      OrcMark const m2 = _orc_sdk_deck_header(actual)->marks[i];
      ORC_SDK_REQUIRE(m1.pos == m2.pos && m1.depth == m2.depth);
    }
    size_t const count = orc_sdk_deck_len(actual);
    ORC_SDK_REQUIRE(count == orc_sdk_deck_len(expected));
    for (size_t i = 0; i < count; ++i) {
      ORC_SDK_REQUIRE(actual[i] == expected[i]);
    }
    orc_sdk_deck_free(expected);
  }
  ORC_SDK_REQUIRE(ORC_ERROR_NONE == orc_sdk_handle_free(&in));
  ORC_SDK_REQUIRE(ORC_ERROR_NONE == orc_sdk_handle_free(&out));
}

void test_two_output_combinations(void)
{
  /*=== Tests multiple-output Combinations: sq+cb (1 in, 2 out) and add+mul (2 in, 2 out).
   * ===*/
  OrcHandle in_a = {0}, in_b = {0}, out1 = {0}, out2 = {0};
  orc_sdk_handle_alloc(ORC_TYPE_F64, &in_a);
  orc_sdk_handle_alloc(ORC_TYPE_F64, &in_b);
  orc_sdk_handle_alloc(ORC_TYPE_F64, &out1);
  orc_sdk_handle_alloc(ORC_TYPE_F64, &out2);
  ORC_SDK_REQUIRE_WITH_MSG(
    in_a.items != NULL && in_b.items != NULL && out1.items != NULL && out2.items != NULL,
    "Unable to allocate decks");

  { /* One input, two outputs: square and cube of 3 scalars (stack_depth=2). */
    ORC_SDK_DECK_INIT(in_a.items, double, (2.0, 3.0, 4.0));
    orc_sdk_oh_update(&in_a);

    _plugin_function_sq_cb(&in_a, &out1, &out2);

    orc_sdk_oh_update(&out1);
    orc_sdk_oh_update(&out2);
    ORC_SDK_REQUIRE(orc_sdk_deck_len(out1.items) == 3);
    ORC_SDK_REQUIRE(orc_sdk_deck_len(out2.items) == 3);
    ORC_SDK_REQUIRE(orc_sdk_deck_max_depth(out1.items) == 1);
    ORC_SDK_REQUIRE(orc_sdk_deck_max_depth(out2.items) == 1);
    double const  expected_sq[] = {4.0, 9.0, 16.0};
    double const  expected_cb[] = {8.0, 27.0, 64.0};
    double *const sq_actual     = (double *)out1.items;
    double *const cb_actual     = (double *)out2.items;
    for (size_t i = 0; i < 3; ++i) {
      ORC_SDK_REQUIRE(sq_actual[i] == expected_sq[i]);
      ORC_SDK_REQUIRE(cb_actual[i] == expected_cb[i]);
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
    ORC_SDK_REQUIRE(orc_sdk_deck_len(out1.items) == 3);
    ORC_SDK_REQUIRE(orc_sdk_deck_len(out2.items) == 3);
    ORC_SDK_REQUIRE(orc_sdk_deck_max_depth(out1.items) == 1);
    ORC_SDK_REQUIRE(orc_sdk_deck_max_depth(out2.items) == 1);
    double const  expected_sum[]  = {5.0, 7.0, 9.0};
    double const  expected_prod[] = {4.0, 10.0, 18.0};
    double *const sum_actual      = (double *)out1.items;
    double *const prod_actual     = (double *)out2.items;
    for (size_t i = 0; i < 3; ++i) {
      ORC_SDK_REQUIRE(sum_actual[i] == expected_sum[i]);
      ORC_SDK_REQUIRE(prod_actual[i] == expected_prod[i]);
    }
  }
  ORC_SDK_REQUIRE(ORC_ERROR_NONE == orc_sdk_handle_free(&in_a));
  ORC_SDK_REQUIRE(ORC_ERROR_NONE == orc_sdk_handle_free(&in_b));
  ORC_SDK_REQUIRE(ORC_ERROR_NONE == orc_sdk_handle_free(&out1));
  ORC_SDK_REQUIRE(ORC_ERROR_NONE == orc_sdk_handle_free(&out2));
}

void test_first_add_combinations(void)
{
  /*=== Tests arg_depth=1: plugin receives depth-1 list views and sums their first
   * elements. ===*/
  OrcHandle a = {0}, b = {0}, out = {0};
  orc_sdk_handle_alloc(ORC_TYPE_F64, &a);
  orc_sdk_handle_alloc(ORC_TYPE_F64, &b);
  orc_sdk_handle_alloc(ORC_TYPE_F64, &out);
  ORC_SDK_REQUIRE_WITH_MSG(a.items != NULL && b.items != NULL && out.items != NULL,
                           "Unable to allocate decks");

  { /* Equal-length: a and b each have 3 depth=1 groups (stack_depth=2). */
    ORC_SDK_DECK_INIT(a.items, double, ((1.0, 99.0), (2.0, 99.0), (3.0, 99.0)));
    orc_sdk_oh_update(&a);
    ORC_SDK_DECK_INIT(b.items, double, ((10.0, 99.0), (20.0, 99.0), (30.0, 99.0)));
    orc_sdk_oh_update(&b);

    _plugin_function_first_add(&a, &b, &out);

    orc_sdk_oh_update(&out);
    size_t const count = orc_sdk_deck_len(out.items);
    ORC_SDK_REQUIRE(count == 3);
    ORC_SDK_REQUIRE(orc_sdk_deck_max_depth(out.items) == 1);
    double const  expected[] = {11.0, 22.0, 33.0};
    double *const actual     = (double *)out.items;
    for (size_t i = 0; i < count; ++i) {
      ORC_SDK_REQUIRE(actual[i] == expected[i]);
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
    ORC_SDK_REQUIRE(count == 4);
    ORC_SDK_REQUIRE(orc_sdk_deck_max_depth(out.items) == 1);
    // b is exhausted after its second group and stays at first(b[1])=20.0.
    double const  expected[] = {11.0, 22.0, 23.0, 24.0};
    double *const actual     = (double *)out.items;
    for (size_t i = 0; i < count; ++i) {
      ORC_SDK_REQUIRE(actual[i] == expected[i]);
    }
  }
  ORC_SDK_REQUIRE(ORC_ERROR_NONE == orc_sdk_handle_free(&a));
  ORC_SDK_REQUIRE(ORC_ERROR_NONE == orc_sdk_handle_free(&b));
  ORC_SDK_REQUIRE(ORC_ERROR_NONE == orc_sdk_handle_free(&out));
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
  ORC_SDK_REQUIRE(na == ne);
  ORC_SDK_REQUIRE(memcmp(actual, expected, na * item_size) == 0);
  ORC_SDK_REQUIRE(orc_sdk_deck_max_depth(actual) == orc_sdk_deck_max_depth(expected));
  _OrcSdk_DeckHeader *ha  = _orc_sdk_deck_header(actual);
  _OrcSdk_DeckHeader *he  = _orc_sdk_deck_header(expected);
  size_t const        nma = orc_sdk_arr_len(ha->marks);
  size_t const        nme = orc_sdk_arr_len(he->marks);
  ORC_SDK_REQUIRE(nma == nme);
  for (size_t i = 0; i < nma; ++i) {
    ORC_SDK_REQUIRE(ha->marks[i].pos == he->marks[i].pos);
    ORC_SDK_REQUIRE(ha->marks[i].depth == he->marks[i].depth);
  }
}

void test_deck_from_proxy_copy_items(void)
{
  /*=== COPY_ITEMS: copies items from input, structure (marks) from proxy. ===*/
  { /* Flatten a depth-2 deck. */
    OrcHandle in = {0}, out = {0};
    orc_sdk_handle_alloc(ORC_TYPE_F64, &in);
    ORC_SDK_DECK_INIT(in.items, double, ((1.0, 2.0), (3.0, 4.0, 5.0)));
    orc_sdk_oh_update(&in);

    OrcHandle proxy = _make_flattened_proxy(in.items);
    orc_sdk_deck_from_proxy(&in, 1, ORC_DECK_PROXY_COPY_ITEMS, &proxy, &out);

    ORC_SDK_REQUIRE(out.type_id == ORC_TYPE_F64);
    ORC_SDK_REQUIRE(orc_sdk_deck_len(out.items) == 5);
    ORC_SDK_REQUIRE(orc_sdk_deck_max_depth(out.items) == 1);
    double *actual = (double *)out.items;
    ORC_SDK_REQUIRE(actual[0] == 1.0 && actual[1] == 2.0 && actual[2] == 3.0);
    ORC_SDK_REQUIRE(actual[3] == 4.0 && actual[4] == 5.0);

    orc_sdk_arr_free(proxy.marks);
    ORC_SDK_REQUIRE(ORC_ERROR_NONE == orc_sdk_handle_free(&out));
    ORC_SDK_REQUIRE(ORC_ERROR_NONE == orc_sdk_handle_free(&in));
  }
  { /* Flatten a depth-3 deck. */
    OrcHandle in = {0}, out = {0};
    orc_sdk_handle_alloc(ORC_TYPE_F64, &in);
    ORC_SDK_DECK_INIT(in.items, double, (((1.0, 2.0), (3.0)), ((4.0, 5.0))));
    orc_sdk_oh_update(&in);

    OrcHandle proxy = _make_flattened_proxy(in.items);
    orc_sdk_deck_from_proxy(&in, 1, ORC_DECK_PROXY_COPY_ITEMS, &proxy, &out);

    ORC_SDK_REQUIRE(orc_sdk_deck_len(out.items) == 5);
    ORC_SDK_REQUIRE(orc_sdk_deck_max_depth(out.items) == 1);
    double *actual = (double *)out.items;
    ORC_SDK_REQUIRE(actual[0] == 1.0 && actual[4] == 5.0);

    orc_sdk_arr_free(proxy.marks);
    ORC_SDK_REQUIRE(ORC_ERROR_NONE == orc_sdk_handle_free(&out));
    ORC_SDK_REQUIRE(ORC_ERROR_NONE == orc_sdk_handle_free(&in));
  }
  { /* Graft a flat deck: (1, 2, 3) → ((1), (2), (3)). */
    OrcHandle in = {0}, out = {0};
    orc_sdk_handle_alloc(ORC_TYPE_F64, &in);
    ORC_SDK_DECK_INIT(in.items, double, (1.0, 2.0, 3.0));
    orc_sdk_oh_update(&in);

    OrcHandle proxy = _make_grafted_proxy(in.items);
    orc_sdk_deck_from_proxy(&in, 1, ORC_DECK_PROXY_COPY_ITEMS, &proxy, &out);

    double *expected = NULL;
    ORC_SDK_DECK_INIT(expected, double, ((1.0), (2.0), (3.0)));
    _assert_decks_match(out.items, expected, sizeof(double));
    ORC_SDK_REQUIRE(out.type_id == ORC_TYPE_F64);

    orc_sdk_deck_free(expected);
    orc_sdk_arr_free(proxy.marks);
    ORC_SDK_REQUIRE(ORC_ERROR_NONE == orc_sdk_handle_free(&out));
    ORC_SDK_REQUIRE(ORC_ERROR_NONE == orc_sdk_handle_free(&in));
  }
  { /* Graft a depth-2 deck: ((1, 2), (3)) → (((1, 2)), ((3))). */
    OrcHandle in = {0}, out = {0};
    orc_sdk_handle_alloc(ORC_TYPE_F64, &in);
    ORC_SDK_DECK_INIT(in.items, double, ((1.0, 2.0), (3.0)));
    orc_sdk_oh_update(&in);

    OrcHandle proxy = _make_grafted_proxy(in.items);
    orc_sdk_deck_from_proxy(&in, 1, ORC_DECK_PROXY_COPY_ITEMS, &proxy, &out);

    double *expected = NULL;
    ORC_SDK_DECK_INIT(expected, double, (((1.0), (2.0)), ((3.0))));
    _assert_decks_match(out.items, expected, sizeof(double));

    orc_sdk_deck_free(expected);
    orc_sdk_arr_free(proxy.marks);
    ORC_SDK_REQUIRE(ORC_ERROR_NONE == orc_sdk_handle_free(&out));
    ORC_SDK_REQUIRE(ORC_ERROR_NONE == orc_sdk_handle_free(&in));
  }
  { /* Simplify: remove gaps in depth levels. */
    OrcHandle in = {0}, out = {0};
    orc_sdk_handle_alloc(ORC_TYPE_F64, &in);
    ORC_SDK_DECK_INIT(in.items, double, ((1.0, 2.0), (3.0, 4.0)));
    orc_sdk_oh_update(&in);
    /* Graft to create a gap in depth levels, then use simplify proxy. */
    orc_sdk_deck_graft((void *)in.items);
    orc_sdk_oh_update(&in);

    OrcHandle proxy = _make_simplified_proxy(in.items);
    orc_sdk_deck_from_proxy(&in, 1, ORC_DECK_PROXY_COPY_ITEMS, &proxy, &out);

    /* Simplify should match orc_sdk_deck_simplify on an equivalent deck. */
    double *expected = NULL;
    ORC_SDK_DECK_INIT(expected, double, ((1.0, 2.0), (3.0, 4.0)));
    orc_sdk_deck_graft(expected);
    orc_sdk_deck_simplify(expected);
    _assert_decks_match(out.items, expected, sizeof(double));

    orc_sdk_deck_free(expected);
    orc_sdk_arr_free(proxy.marks);
    ORC_SDK_REQUIRE(ORC_ERROR_NONE == orc_sdk_handle_free(&out));
    ORC_SDK_REQUIRE(ORC_ERROR_NONE == orc_sdk_handle_free(&in));
  }
}

void test_deck_from_proxy_shuffle(void)
{
  /*=== SHUFFLE: copies items one-at-a-time using proxy ItemProxy references. ===*/
  { /* Flat reverse: (1, 2, 3) → (3, 2, 1). */
    OrcHandle in = {0}, out = {0};
    orc_sdk_handle_alloc(ORC_TYPE_F64, &in);
    ORC_SDK_DECK_INIT(in.items, double, (1.0, 2.0, 3.0));
    orc_sdk_oh_update(&in);

    OrcItemProxy *pdeck = NULL;
    ORC_SDK_REQUIRE(
      orc_sdk_deck_push(pdeck, ((OrcItemProxy) {.tree = 0, .item = 2}), 1) == OK);
    ORC_SDK_REQUIRE(
      orc_sdk_deck_push(pdeck, ((OrcItemProxy) {.tree = 0, .item = 1}), 0) == OK);
    ORC_SDK_REQUIRE(
      orc_sdk_deck_push(pdeck, ((OrcItemProxy) {.tree = 0, .item = 0}), 0) == OK);
    OrcHandle proxy = _make_shuffle_proxy(pdeck);
    orc_sdk_deck_from_proxy(&in, 1, ORC_DECK_PROXY_SHUFFLE, &proxy, &out);

    double *expected = NULL;
    ORC_SDK_DECK_INIT(expected, double, (3.0, 2.0, 1.0));
    _assert_decks_match(out.items, expected, sizeof(double));
    ORC_SDK_REQUIRE(out.type_id == ORC_TYPE_F64);

    orc_sdk_deck_free(expected);
    orc_sdk_deck_free(pdeck);
    ORC_SDK_REQUIRE(ORC_ERROR_NONE == orc_sdk_handle_free(&out));
    ORC_SDK_REQUIRE(ORC_ERROR_NONE == orc_sdk_handle_free(&in));
  }
  { /* Nested reverse: ((1, 2), (3, 4, 5)) → ((2, 1), (5, 4, 3)). */
    OrcHandle in = {0}, out = {0};
    orc_sdk_handle_alloc(ORC_TYPE_F64, &in);
    ORC_SDK_DECK_INIT(in.items, double, ((1.0, 2.0), (3.0, 4.0, 5.0)));
    orc_sdk_oh_update(&in);

    OrcItemProxy *pdeck = NULL;
    /* First sublist reversed: flat indices 1, 0. */
    ORC_SDK_REQUIRE(
      orc_sdk_deck_push(pdeck, ((OrcItemProxy) {.tree = 0, .item = 1}), 2) == OK);
    ORC_SDK_REQUIRE(
      orc_sdk_deck_push(pdeck, ((OrcItemProxy) {.tree = 0, .item = 0}), 0) == OK);
    /* Second sublist reversed: flat indices 4, 3, 2. */
    ORC_SDK_REQUIRE(
      orc_sdk_deck_push(pdeck, ((OrcItemProxy) {.tree = 0, .item = 4}), 1) == OK);
    ORC_SDK_REQUIRE(
      orc_sdk_deck_push(pdeck, ((OrcItemProxy) {.tree = 0, .item = 3}), 0) == OK);
    ORC_SDK_REQUIRE(
      orc_sdk_deck_push(pdeck, ((OrcItemProxy) {.tree = 0, .item = 2}), 0) == OK);
    OrcHandle proxy = _make_shuffle_proxy(pdeck);
    orc_sdk_deck_from_proxy(&in, 1, ORC_DECK_PROXY_SHUFFLE, &proxy, &out);

    double *expected = NULL;
    ORC_SDK_DECK_INIT(expected, double, ((2.0, 1.0), (5.0, 4.0, 3.0)));
    _assert_decks_match(out.items, expected, sizeof(double));

    orc_sdk_deck_free(expected);
    orc_sdk_deck_free(pdeck);
    ORC_SDK_REQUIRE(ORC_ERROR_NONE == orc_sdk_handle_free(&out));
    ORC_SDK_REQUIRE(ORC_ERROR_NONE == orc_sdk_handle_free(&in));
  }
  { /* Generic list_item: pick item at index 1 from each sublist.
       ((1, 2, 3), (4, 5)) → (2, 5). */
    OrcHandle in = {0}, out = {0};
    orc_sdk_handle_alloc(ORC_TYPE_F64, &in);
    ORC_SDK_DECK_INIT(in.items, double, ((1.0, 2.0, 3.0), (4.0, 5.0)));
    orc_sdk_oh_update(&in);

    OrcItemProxy *pdeck = NULL;
    ORC_SDK_REQUIRE(
      orc_sdk_deck_push(pdeck, ((OrcItemProxy) {.tree = 0, .item = 1}), 1) == OK);
    ORC_SDK_REQUIRE(
      orc_sdk_deck_push(pdeck, ((OrcItemProxy) {.tree = 0, .item = 4}), 0) == OK);
    OrcHandle proxy = _make_shuffle_proxy(pdeck);
    orc_sdk_deck_from_proxy(&in, 1, ORC_DECK_PROXY_SHUFFLE, &proxy, &out);

    ORC_SDK_REQUIRE(out.type_id == ORC_TYPE_F64);
    ORC_SDK_REQUIRE(orc_sdk_deck_len(out.items) == 2);
    double *actual = (double *)out.items;
    ORC_SDK_REQUIRE(actual[0] == 2.0 && actual[1] == 5.0);

    orc_sdk_deck_free(pdeck);
    ORC_SDK_REQUIRE(ORC_ERROR_NONE == orc_sdk_handle_free(&out));
    ORC_SDK_REQUIRE(ORC_ERROR_NONE == orc_sdk_handle_free(&in));
  }
  { /* Multi-input shuffle: interleave from two decks.
       A=(1, 2), B=(10, 20) → (1, 10, 2, 20). */
    OrcHandle a = {0}, b = {0}, out = {0};
    orc_sdk_handle_alloc(ORC_TYPE_F64, &a);
    orc_sdk_handle_alloc(ORC_TYPE_F64, &b);
    ORC_SDK_DECK_INIT(a.items, double, (1.0, 2.0));
    orc_sdk_oh_update(&a);
    ORC_SDK_DECK_INIT(b.items, double, (10.0, 20.0));
    orc_sdk_oh_update(&b);

    OrcItemProxy *pdeck = NULL;
    ORC_SDK_REQUIRE(
      orc_sdk_deck_push(pdeck, ((OrcItemProxy) {.tree = 0, .item = 0}), 1) == OK);
    ORC_SDK_REQUIRE(
      orc_sdk_deck_push(pdeck, ((OrcItemProxy) {.tree = 1, .item = 0}), 0) == OK);
    ORC_SDK_REQUIRE(
      orc_sdk_deck_push(pdeck, ((OrcItemProxy) {.tree = 0, .item = 1}), 0) == OK);
    ORC_SDK_REQUIRE(
      orc_sdk_deck_push(pdeck, ((OrcItemProxy) {.tree = 1, .item = 1}), 0) == OK);
    OrcHandle proxy = _make_shuffle_proxy(pdeck);

    OrcHandle inputs[2] = {a, b};
    orc_sdk_deck_from_proxy(inputs, 2, ORC_DECK_PROXY_SHUFFLE, &proxy, &out);

    double *expected = NULL;
    ORC_SDK_DECK_INIT(expected, double, (1.0, 10.0, 2.0, 20.0));
    _assert_decks_match(out.items, expected, sizeof(double));

    orc_sdk_deck_free(expected);
    orc_sdk_deck_free(pdeck);
    ORC_SDK_REQUIRE(ORC_ERROR_NONE == orc_sdk_handle_free(&out));
    ORC_SDK_REQUIRE(ORC_ERROR_NONE == orc_sdk_handle_free(&b));
    ORC_SDK_REQUIRE(ORC_ERROR_NONE == orc_sdk_handle_free(&a));
  }
}

void test_deck_from_proxy_type_agnostic(void)
{
  /*=== Verifies orc_deck_from_proxy preserves type across u32, i32, i16. ===*/
  { /* u32 flatten. */
    OrcHandle in = {0}, out = {0};
    orc_sdk_handle_alloc(ORC_TYPE_U32, &in);
    ORC_SDK_DECK_INIT(in.items, uint32_t, ((10, 20), (30)));
    orc_sdk_oh_update(&in);

    OrcHandle proxy = _make_flattened_proxy(in.items);
    orc_sdk_deck_from_proxy(&in, 1, ORC_DECK_PROXY_COPY_ITEMS, &proxy, &out);

    ORC_SDK_REQUIRE(out.type_id == ORC_TYPE_U32);
    ORC_SDK_REQUIRE(orc_sdk_deck_len(out.items) == 3);
    ORC_SDK_REQUIRE(orc_sdk_deck_max_depth(out.items) == 1);
    uint32_t *actual = (uint32_t *)out.items;
    ORC_SDK_REQUIRE(actual[0] == 10 && actual[1] == 20 && actual[2] == 30);

    orc_sdk_arr_free(proxy.marks);
    ORC_SDK_REQUIRE(ORC_ERROR_NONE == orc_sdk_handle_free(&out));
    ORC_SDK_REQUIRE(ORC_ERROR_NONE == orc_sdk_handle_free(&in));
  }
  { /* i32 shuffle reverse. */
    OrcHandle in = {0}, out = {0};
    orc_sdk_handle_alloc(ORC_TYPE_I32, &in);
    ORC_SDK_DECK_INIT(in.items, int32_t, (-1, -2, -3, -4));
    orc_sdk_oh_update(&in);

    OrcItemProxy *pdeck = NULL;
    ORC_SDK_REQUIRE(
      orc_sdk_deck_push(pdeck, ((OrcItemProxy) {.tree = 0, .item = 3}), 1) == OK);
    ORC_SDK_REQUIRE(
      orc_sdk_deck_push(pdeck, ((OrcItemProxy) {.tree = 0, .item = 2}), 0) == OK);
    ORC_SDK_REQUIRE(
      orc_sdk_deck_push(pdeck, ((OrcItemProxy) {.tree = 0, .item = 1}), 0) == OK);
    ORC_SDK_REQUIRE(
      orc_sdk_deck_push(pdeck, ((OrcItemProxy) {.tree = 0, .item = 0}), 0) == OK);
    OrcHandle proxy = _make_shuffle_proxy(pdeck);
    orc_sdk_deck_from_proxy(&in, 1, ORC_DECK_PROXY_SHUFFLE, &proxy, &out);

    ORC_SDK_REQUIRE(out.type_id == ORC_TYPE_I32);
    ORC_SDK_REQUIRE(orc_sdk_deck_len(out.items) == 4);
    int32_t *actual = (int32_t *)out.items;
    ORC_SDK_REQUIRE(actual[0] == -4 && actual[1] == -3 && actual[2] == -2 &&
                    actual[3] == -1);

    orc_sdk_deck_free(pdeck);
    ORC_SDK_REQUIRE(ORC_ERROR_NONE == orc_sdk_handle_free(&out));
    ORC_SDK_REQUIRE(ORC_ERROR_NONE == orc_sdk_handle_free(&in));
  }
  { /* i16 graft. */
    OrcHandle in = {0}, out = {0};
    orc_sdk_handle_alloc(ORC_TYPE_I16, &in);
    ORC_SDK_DECK_INIT(in.items, int16_t, (10, 20, 30));
    orc_sdk_oh_update(&in);

    OrcHandle proxy = _make_grafted_proxy(in.items);
    orc_sdk_deck_from_proxy(&in, 1, ORC_DECK_PROXY_COPY_ITEMS, &proxy, &out);

    ORC_SDK_REQUIRE(out.type_id == ORC_TYPE_I16);
    ORC_SDK_REQUIRE(orc_sdk_deck_len(out.items) == 3);
    ORC_SDK_REQUIRE(orc_sdk_deck_max_depth(out.items) == 2);
    int16_t *actual = (int16_t *)out.items;
    ORC_SDK_REQUIRE(actual[0] == 10 && actual[1] == 20 && actual[2] == 30);

    orc_sdk_arr_free(proxy.marks);
    ORC_SDK_REQUIRE(ORC_ERROR_NONE == orc_sdk_handle_free(&out));
    ORC_SDK_REQUIRE(ORC_ERROR_NONE == orc_sdk_handle_free(&in));
  }
}
