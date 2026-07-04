#include "orc_sdk.h"

#include <inttypes.h>
#include <limits.h>
#include <stddef.h>
#include <stdio.h>
#include <string.h>

void test_arr_null_pointer_operations(void)
{
  double* null_arr = NULL;
  REQUIRE_WITH_MSG(arr_len(null_arr) == 0, "Null pointer represents an empty array");
  REQUIRE_WITH_MSG(arr_end(null_arr) == null_arr, "End of a NULL is itself");
  REQUIRE_WITH_MSG(arr_swap_remove(null_arr, 0) == OUT_OF_BOUNDS,
                   "Cannot remove from empty array");
  double* arr = NULL;
  REQUIRE_WITH_MSG(arr_reserve(arr, 10) == OK, "Reserve starting with NULL");
  REQUIRE_WITH_MSG(arr != NULL, "Should be allocated after reserve");
  arr_free(arr);
  // Should not crash
  double* ptr = NULL;
  arr_free(ptr);
}

void test_arr_empty_array_operations(void)
{
  double* arr = NULL;
  REQUIRE_WITH_MSG(arr_reserve(arr, 0) == OK, "Empty array reserve");
  REQUIRE_WITH_MSG(arr_len(arr) == 0, "Length after reserved is zero");
  REQUIRE_WITH_MSG(arr_swap_remove(arr, 0) == OUT_OF_BOUNDS,
                   "Cannot remove from empty array");
  REQUIRE_WITH_MSG(arr_push(arr, 1.0) == OK, "Push into empty array");
  REQUIRE_WITH_MSG(arr_len(arr) == 1, "Array after pushing");
  arr_free(arr);
}

void test_arr_index_boundary_conditions(void)
{
  double* arr = NULL;
  arr_push(arr, 1.0);
  // Single element array
  REQUIRE(arr_swap_remove(arr, 0) == OK);
  REQUIRE(arr_len(arr) == 0);
  // Empty array
  REQUIRE(arr_swap_remove(arr, 0) == OUT_OF_BOUNDS);
  // Add elements back
  arr_push(arr, 1.0);
  arr_push(arr, 2.0);
  // One past end
  REQUIRE(arr_swap_remove(arr, arr_len(arr)) == OUT_OF_BOUNDS);
  // Way past end
  REQUIRE(arr_swap_remove(arr, arr_len(arr) + 10) == OUT_OF_BOUNDS);
  // Huge index
  REQUIRE(arr_swap_remove(arr, SIZE_MAX) == OUT_OF_BOUNDS);
  arr_free(arr);
}

void test_arr_capacity_management(void)
{
  double* arr = NULL;
  // Reserve initial capacity
  REQUIRE(arr_reserve(arr, 4) == OK);
  REQUIRE(_arr_capacity(arr) == 4);
  // Reserve smaller (should be no-op)
  size_t old_cap = _arr_capacity(arr);
  REQUIRE(arr_reserve(arr, 2) == OK);
  REQUIRE(_arr_capacity(arr) == old_cap);
  // Reserve exact current capacity (should be no-op)
  REQUIRE(arr_reserve(arr, old_cap) == OK);
  REQUIRE(_arr_capacity(arr) == old_cap);
  // Reserve larger
  REQUIRE(arr_reserve(arr, 10) == OK);
  REQUIRE(_arr_capacity(arr) == 10);
  // Test growth pattern
  arr_free(arr);
  arr = NULL;
  REQUIRE(arr_push(arr, 1.0) == OK);
  size_t cap1 = _arr_capacity(arr);
  // Fill to capacity
  while (arr_len(arr) < cap1) {
    arr_push(arr, (double)arr_len(arr));
  }
  // Next push should grow
  arr_push(arr, 999.0);
  REQUIRE(_arr_capacity(arr) > cap1);
  arr_free(arr);
}

void test_arr_double_free_safety(void)
{
  double* arr = NULL;
  arr_push(arr, 1.0);
  arr_free(arr);  // First free, arr becomes NULL
  arr_free(arr);  // Second free on NULL, should not crash
}

void test_arr_swap_remove_correctness(void)
{
  double* arr = NULL;
  // Setup: [0, 1, 2, 3, 4]
  for (int i = 0; i < 5; i++) {
    arr_push(arr, (double)i);
  }
  // Remove middle element (index 2, value 2.0)
  // Should replace with last element (value 4.0)
  REQUIRE(arr_swap_remove(arr, 2) == OK);
  REQUIRE(arr_len(arr) == 4);
  REQUIRE(arr[2] == 4.0);  // Last element moved here
  // Remove first element
  REQUIRE(arr_swap_remove(arr, 0) == OK);
  REQUIRE(arr_len(arr) == 3);
  REQUIRE(arr[0] == 3.0);  // Last element moved to front
  // Remove last element
  size_t last_idx = arr_len(arr) - 1;
  REQUIRE(arr_swap_remove(arr, last_idx) == OK);
  REQUIRE(arr_len(arr) == 2);
  // Remove from single-element array
  arr_swap_remove(arr, 0);
  arr_swap_remove(arr, 0);
  REQUIRE(arr_len(arr) == 0);
  arr_free(arr);
}

void test_arr_memory_stress(void)
{
  double*      arr        = NULL;
  const size_t LARGE_SIZE = 1000;
  // Push many elements
  for (size_t i = 0; i < LARGE_SIZE; i++) {
    REQUIRE(arr_push(arr, (double)i) == OK);
  }
  // Verify all elements
  REQUIRE(arr_len(arr) == LARGE_SIZE);
  for (size_t i = 0; i < LARGE_SIZE; i++) {
    REQUIRE(arr[i] == (double)i);
  }
  // Repeated push/remove cycles
  for (int cycle = 0; cycle < 100; cycle++) {
    size_t old_len = arr_len(arr);
    arr_push(arr, 999.0);
    REQUIRE(arr_len(arr) == old_len + 1);
    arr_swap_remove(arr, arr_len(arr) - 1);
    REQUIRE(arr_len(arr) == old_len);
  }
  arr_free(arr);
}

void test_arr_different_types(void)
{
  // Test with int
  int* ints = NULL;
  REQUIRE(arr_push(ints, 42) == OK);
  REQUIRE(arr_push(ints, -17) == OK);
  REQUIRE(arr_len(ints) == 2);
  REQUIRE(ints[0] == 42);
  REQUIRE(ints[1] == -17);
  arr_free(ints);
  // Test with char
  char* chars = NULL;
  REQUIRE(arr_push(chars, 'A') == OK);
  REQUIRE(arr_push(chars, 'B') == OK);
  REQUIRE(chars[0] == 'A');
  REQUIRE(chars[1] == 'B');
  arr_free(chars);
  // Test with pointers
  const char*  strings[] = {"hello", "world"};
  const char** str_ptrs  = NULL;
  REQUIRE(arr_push(str_ptrs, strings[0]) == OK);
  REQUIRE(arr_push(str_ptrs, strings[1]) == OK);
  REQUIRE(str_ptrs[0] == strings[0]);
  REQUIRE(str_ptrs[1] == strings[1]);
  arr_free(str_ptrs);
  // Test with struct
  typedef struct
  {
    int    x, y;
    double value;
  } Point;
  Point* points = NULL;
  Point  p1     = {10, 20, 3.14};
  Point  p2     = {-5, 15, 2.71};
  REQUIRE(arr_push(points, p1) == OK);
  REQUIRE(arr_push(points, p2) == OK);
  REQUIRE(points[0].x == 10);
  REQUIRE(points[0].y == 20);
  REQUIRE(points[0].value == 3.14);
  REQUIRE(points[1].x == -5);
  REQUIRE(points[1].y == 15);
  REQUIRE(points[1].value == 2.71);
  arr_free(points);
  // Test alignment by checking data pointer alignment
  long long* longs = NULL;
  REQUIRE(arr_push(longs, 123456789LL) == OK);
  // Check that data pointer is properly aligned for long long
  uintptr_t addr = (uintptr_t)longs;
  REQUIRE_WITH_MSG(addr % sizeof(long long) == 0, "long long array not properly aligned");
  arr_free(longs);
}

void test_arr_ordered_remove(void)
{
  int* arr = NULL;
  // Setup: [10, 20, 30, 40, 50]
  for (int i = 1; i <= 5; i++) {
    REQUIRE(arr_push(arr, i * 10) == OK);
  }
  // Remove middle element (index 2, value 30)
  // Should shift [40, 50] left to fill the gap
  REQUIRE(arr_remove(arr, 2) == OK);
  REQUIRE(arr_len(arr) == 4);
  REQUIRE_WITH_MSG(arr[0] == 10, "First element should be unchanged");
  REQUIRE_WITH_MSG(arr[1] == 20, "Second element should be unchanged");
  REQUIRE_WITH_MSG(arr[2] == 40, "Third element should be 40 (was 4th)");
  REQUIRE_WITH_MSG(arr[3] == 50, "Fourth element should be 50 (was 5th)");
  // Remove first element
  // Should shift [20, 40, 50] left
  REQUIRE(arr_remove(arr, 0) == OK);
  REQUIRE(arr_len(arr) == 3);
  REQUIRE_WITH_MSG(arr[0] == 20, "First element should now be 20");
  REQUIRE_WITH_MSG(arr[1] == 40, "Second element should be 40");
  REQUIRE_WITH_MSG(arr[2] == 50, "Third element should be 50");
  // Remove last element
  // Should just decrease count, no shifting needed
  REQUIRE(arr_remove(arr, 2) == OK);
  REQUIRE(arr_len(arr) == 2);
  REQUIRE_WITH_MSG(arr[0] == 20, "First element unchanged");
  REQUIRE_WITH_MSG(arr[1] == 40, "Second element unchanged");
  // Remove from single-element array
  arr_remove(arr, 0);
  arr_remove(arr, 0);
  REQUIRE_WITH_MSG(arr_len(arr) == 0, "Array should be empty");
  // Test bounds checking
  REQUIRE_WITH_MSG(arr_remove(arr, 0) == OUT_OF_BOUNDS,
                   "Remove from empty array should fail");
  // Add one element and test invalid indices
  arr_push(arr, 100);
  REQUIRE_WITH_MSG(arr_remove(arr, 1) == OUT_OF_BOUNDS, "Remove past end should fail");
  REQUIRE_WITH_MSG(arr_remove(arr, SIZE_MAX) == OUT_OF_BOUNDS,
                   "Remove huge index should fail");
  arr_free(arr);
  // Test with NULL pointer
  int* null_arr = NULL;
  REQUIRE_WITH_MSG(arr_remove(null_arr, 0) == OUT_OF_BOUNDS,
                   "Remove from NULL should fail");
}

void test_arr_resize_zero_fill(void)
{
  double* arr = NULL;
  // Test resize from empty array - should zero-fill all elements
  arr_resize(arr, 5);
  REQUIRE_WITH_MSG(arr != NULL, "Resize from empty should succeed");
  REQUIRE_WITH_MSG(arr_len(arr) == 5, "Array should have 5 elements");
  for (size_t i = 0; i < 5; i++) {
    REQUIRE_WITH_MSG(arr[i] == 0.0, "All elements should be zero-initialized");
  }
  // Fill array with known values to test growth behavior
  for (size_t i = 0; i < 5; i++) {
    arr[i] = (double)(i + 10);  // [10, 11, 12, 13, 14]
  }
  // Test resize to larger size (growth) - new elements should be zero
  arr_resize(arr, 8);
  REQUIRE_WITH_MSG(arr != NULL, "Resize growth should succeed");
  REQUIRE_WITH_MSG(arr_len(arr) == 8, "Array should have 8 elements");
  REQUIRE_WITH_MSG(_arr_capacity(arr) >= 8, "Capacity should accommodate new size");
  // Check original elements unchanged
  for (size_t i = 0; i < 5; i++) {
    REQUIRE_WITH_MSG(arr[i] == (double)(i + 10), "Original elements should be unchanged");
  }
  // Check new elements are zero-filled
  for (size_t i = 5; i < 8; i++) {
    REQUIRE_WITH_MSG(arr[i] == 0.0, "New elements should be zero-initialized");
  }
  // Test resize to smaller size (shrink) - remaining elements preserved
  arr_resize(arr, 3);
  REQUIRE_WITH_MSG(arr != NULL, "Resize shrink should succeed");
  REQUIRE_WITH_MSG(arr_len(arr) == 3, "Array should have 3 elements");
  // Capacity should remain the same (no reallocation on shrink)
  REQUIRE_WITH_MSG(_arr_capacity(arr) >= 8, "Capacity should not decrease on shrink");
  // Check remaining elements unchanged
  for (size_t i = 0; i < 3; i++) {
    REQUIRE_WITH_MSG(arr[i] == (double)(i + 10),
                     "Remaining elements should be unchanged");
  }
  // Test resize to same size (no-op)
  size_t len_before = arr_len(arr);
  size_t cap_before = _arr_capacity(arr);
  arr_resize(arr, 3);
  REQUIRE_WITH_MSG(arr != NULL, "Resize same size should succeed");
  REQUIRE_WITH_MSG(arr_len(arr) == len_before, "Length should be unchanged");
  REQUIRE_WITH_MSG(_arr_capacity(arr) == cap_before, "Capacity should be unchanged");
  for (size_t i = 0; i < 3; i++) {
    REQUIRE_WITH_MSG(arr[i] == (double)(i + 10), "Elements should be unchanged");
  }
  // Test resize to zero (empty)
  arr_resize(arr, 0);
  REQUIRE_WITH_MSG(arr != NULL, "Resize to zero should succeed");
  REQUIRE_WITH_MSG(arr_len(arr) == 0, "Array should be empty");
  REQUIRE_WITH_MSG(_arr_capacity(arr) >= 8, "Capacity should be preserved");
  // Test resize from zero back to non-zero - should zero-fill
  arr_resize(arr, 4);
  REQUIRE_WITH_MSG(arr != NULL, "Resize from zero should succeed");
  REQUIRE_WITH_MSG(arr_len(arr) == 4, "Array should have 4 elements");
  for (size_t i = 0; i < 4; i++) {
    REQUIRE_WITH_MSG(arr[i] == 0.0, "Elements should be zero-initialized");
  }
  arr_free(arr);
  // Test resize with NULL array - should create and zero-fill
  double* null_arr = NULL;
  arr_resize(null_arr, 3);
  REQUIRE_WITH_MSG(null_arr != NULL, "Resize NULL array should succeed");
  REQUIRE_WITH_MSG(arr_len(null_arr) == 3, "Array should have 3 elements");
  for (size_t i = 0; i < 3; i++) {
    REQUIRE_WITH_MSG(null_arr[i] == 0.0, "Elements should be zero-initialized");
  }
  arr_free(null_arr);
  // Test with different types to ensure zero-initialization works correctly
  int* int_arr = NULL;
  arr_resize(int_arr, 3);
  REQUIRE_WITH_MSG(int_arr != NULL, "Int array resize should succeed");
  for (size_t i = 0; i < 3; i++) {
    REQUIRE_WITH_MSG(int_arr[i] == 0, "Int elements should be zero-initialized");
  }
  arr_free(int_arr);
  // Test with pointers
  void** ptr_arr = NULL;
  arr_resize(ptr_arr, 2);
  REQUIRE_WITH_MSG(ptr_arr != NULL, "Pointer array resize should succeed");
  for (size_t i = 0; i < 2; i++) {
    REQUIRE_WITH_MSG(ptr_arr[i] == NULL, "Pointer elements should be NULL-initialized");
  }
  arr_free(ptr_arr);
  // Test large resize to verify performance and correctness
  double*      large_arr  = NULL;
  const size_t LARGE_SIZE = 10000;
  arr_resize(large_arr, LARGE_SIZE);
  REQUIRE_WITH_MSG(large_arr != NULL, "Large resize should succeed");
  REQUIRE_WITH_MSG(arr_len(large_arr) == LARGE_SIZE,
                   "Large array should have correct length");
  // Spot check zero-initialization (checking all would be slow)
  REQUIRE_WITH_MSG(large_arr[0] == 0.0, "First element should be zero");
  REQUIRE_WITH_MSG(large_arr[LARGE_SIZE / 2] == 0.0, "Middle element should be zero");
  REQUIRE_WITH_MSG(large_arr[LARGE_SIZE - 1] == 0.0, "Last element should be zero");
  arr_free(large_arr);
  // Test edge case: resize to 1 element
  double* single_arr = NULL;
  arr_resize(single_arr, 1);
  REQUIRE_WITH_MSG(single_arr != NULL, "Single element resize should succeed");
  REQUIRE_WITH_MSG(arr_len(single_arr) == 1, "Array should have 1 element");
  REQUIRE_WITH_MSG(single_arr[0] == 0.0, "Single element should be zero-initialized");
  // Modify the element then resize larger
  single_arr[0] = 42.0;
  arr_resize(single_arr, 3);
  REQUIRE_WITH_MSG(single_arr[0] == 42.0, "Original element should be preserved");
  REQUIRE_WITH_MSG(single_arr[1] == 0.0, "New elements should be zero-initialized");
  REQUIRE_WITH_MSG(single_arr[2] == 0.0, "New elements should be zero-initialized");
  arr_free(single_arr);
}

void test_arr_fill(void)
{
  // Test 1: Basic fill with integers (power of 2 size)
  int* ints = NULL;
  arr_resize(ints, 4);
  int val = 42;
  arr_fill(ints, val);
  REQUIRE(arr_len(ints) == 4);
  for (size_t i = 0; i < 4; i++) {
    REQUIRE_WITH_MSG(ints[i] == 42, "All elements should be 42");
  }
  // Test 2: Non-power of 2 size
  arr_resize(ints, 7);
  val = 77;
  arr_fill(ints, val);
  REQUIRE(arr_len(ints) == 7);
  for (size_t i = 0; i < 7; i++) {
    REQUIRE_WITH_MSG(ints[i] == 77, "All elements should be 77");
  }
  // Test 3: Large size (to test doubling logic efficiency/correctness)
  arr_resize(ints, 1025);
  val = 123;
  arr_fill(ints, val);
  REQUIRE(arr_len(ints) == 1025);
  for (size_t i = 0; i < 1025; i++) {
    REQUIRE_WITH_MSG(ints[i] == 123, "All elements should be 123");
  }
  arr_free(ints);
  // Test 4: Single element
  double* doubles = NULL;
  arr_resize(doubles, 1);
  double dval = 3.14;
  arr_fill(doubles, dval);
  REQUIRE(arr_len(doubles) == 1);
  REQUIRE(doubles[0] == 3.14);
  arr_free(doubles);
  // Test 5: Empty array
  float* floats = NULL;
  dval          = 1.0f;
  arr_fill(floats, dval);  // arr_len(NULL) is 0
  REQUIRE(arr_len(floats) == 0);
  arr_free(floats);
  // Test 6: Different types (Structs)
  typedef struct
  {
    int    a;
    double b;
  } TestStruct;
  TestStruct* structs = NULL;
  TestStruct  sval    = {10, 20.0};
  arr_resize(structs, 3);
  arr_fill(structs, sval);
  REQUIRE(arr_len(structs) == 3);
  for (size_t i = 0; i < 3; i++) {
    REQUIRE(structs[i].a == 10);
    REQUIRE(structs[i].b == 20.0);
  }
  arr_free(structs);
}

void test_arr_clear(void)
{
  double* arr = NULL;
  // Test clear on empty array
  arr_clear(arr);
  REQUIRE_WITH_MSG(arr_len(arr) == 0, "Clear on NULL array should work");
  // Add some elements
  arr_resize(arr, 5);
  REQUIRE_WITH_MSG(arr_len(arr) == 5, "Array should have 5 elements");
  size_t old_capacity = _arr_capacity(arr);
  // Clear the array
  arr_clear(arr);
  REQUIRE_WITH_MSG(arr_len(arr) == 0, "Array should be empty after clear");
  REQUIRE_WITH_MSG(_arr_capacity(arr) == old_capacity, "Capacity should be preserved");
  // Verify we can still use the array after clear
  Status s = arr_push(arr, 2.71);
  REQUIRE_WITH_MSG(s == OK, "Should be able to push after clear");
  REQUIRE_WITH_MSG(arr_len(arr) == 1, "Array should have 1 element after push");
  REQUIRE_WITH_MSG(arr[0] == 2.71, "Element should be correct");
  // Clear again with elements
  arr_push(arr, 1.0);
  arr_push(arr, 2.0);
  REQUIRE_WITH_MSG(arr_len(arr) == 3, "Array should have 3 elements");
  arr_clear(arr);
  REQUIRE_WITH_MSG(arr_len(arr) == 0, "Array should be empty after second clear");
  // Test operations on cleared array
  REQUIRE_WITH_MSG(arr_swap_remove(arr, 0) == OUT_OF_BOUNDS,
                   "Remove from cleared array should fail");
  arr_free(arr);
  // Test clear on NULL pointer (should not crash)
  double* null_arr = NULL;
  arr_clear(null_arr);  // Should not crash
}

void test_arr_remove_range(void)
{
  int* arr = NULL;
  // Setup test array: [10, 20, 30, 40, 50]
  for (int i = 1; i <= 5; i++) {
    Status s = arr_push(arr, i * 10);
    REQUIRE_WITH_MSG(s == OK, "Setup should succeed");
  }
  REQUIRE_WITH_MSG(arr_len(arr) == 5, "Array should have 5 elements");
  // Test 1: Remove middle range [1, 3) -> removes 20, 30
  // Expected: [10, 40, 50]
  Status result = arr_remove_range(arr, 1, 3);
  REQUIRE_WITH_MSG(result == OK, "Remove middle range should succeed");
  REQUIRE_WITH_MSG(arr_len(arr) == 3, "Array should have 3 elements after removing 2");
  REQUIRE_WITH_MSG(arr[0] == 10, "First element should be unchanged");
  REQUIRE_WITH_MSG(arr[1] == 40, "Second element should be 40 (was 4th)");
  REQUIRE_WITH_MSG(arr[2] == 50, "Third element should be 50 (was 5th)");
  // Test 2: Remove from beginning [0, 1) -> removes 10
  // Expected: [40, 50]
  result = arr_remove_range(arr, 0, 1);
  REQUIRE_WITH_MSG(result == OK, "Remove from beginning should succeed");
  REQUIRE_WITH_MSG(arr_len(arr) == 2, "Array should have 2 elements");
  REQUIRE_WITH_MSG(arr[0] == 40, "First element should be 40");
  REQUIRE_WITH_MSG(arr[1] == 50, "Second element should be 50");
  // Test 3: Remove from end [1, 2) -> removes 50
  // Expected: [40]
  result = arr_remove_range(arr, 1, 2);
  REQUIRE_WITH_MSG(result == OK, "Remove from end should succeed");
  REQUIRE_WITH_MSG(arr_len(arr) == 1, "Array should have 1 element");
  REQUIRE_WITH_MSG(arr[0] == 40, "Remaining element should be 40");
  // Test 4: Remove entire array [0, 1)
  result = arr_remove_range(arr, 0, 1);
  REQUIRE_WITH_MSG(result == OK, "Remove entire array should succeed");
  REQUIRE_WITH_MSG(arr_len(arr) == 0, "Array should be empty");
  // Test 5: Empty range operations
  // Add elements back
  arr_push(arr, 100);
  arr_push(arr, 200);
  arr_push(arr, 300);
  REQUIRE_WITH_MSG(arr_len(arr) == 3, "Array should have 3 elements");
  // Remove empty range at beginning [0, 0)
  result = arr_remove_range(arr, 0, 0);
  REQUIRE_WITH_MSG(result == OK, "Empty range at beginning should succeed");
  REQUIRE_WITH_MSG(arr_len(arr) == 3, "Array length should be unchanged");
  // Remove empty range in middle [1, 1)
  result = arr_remove_range(arr, 1, 1);
  REQUIRE_WITH_MSG(result == OK, "Empty range in middle should succeed");
  REQUIRE_WITH_MSG(arr_len(arr) == 3, "Array length should be unchanged");
  // Remove empty range at end [3, 3)
  result = arr_remove_range(arr, 3, 3);
  REQUIRE_WITH_MSG(result == OK, "Empty range at end should succeed");
  REQUIRE_WITH_MSG(arr_len(arr) == 3, "Array length should be unchanged");
  // Test 6: Error cases - out of bounds
  // Start index too large
  result = arr_remove_range(arr, 4, 4);
  REQUIRE_WITH_MSG(result == OUT_OF_BOUNDS, "Start beyond array should fail");
  // Stop index too large
  result = arr_remove_range(arr, 1, 5);
  REQUIRE_WITH_MSG(result == OUT_OF_BOUNDS, "Stop beyond array should fail");
  // Invalid range (stop < start)
  result = arr_remove_range(arr, 2, 1);
  REQUIRE_WITH_MSG(result == OUT_OF_BOUNDS, "Invalid range should fail");
  // Test 7: Remove everything [0, length)
  result = arr_remove_range(arr, 0, arr_len(arr));
  REQUIRE_WITH_MSG(result == OK, "Remove all elements should succeed");
  REQUIRE_WITH_MSG(arr_len(arr) == 0, "Array should be empty");
  arr_free(arr);
  // Test 8: Operations on NULL array
  int* null_arr = NULL;
  result        = arr_remove_range(null_arr, 0, 0);
  REQUIRE_WITH_MSG(result == OUT_OF_BOUNDS, "Remove from NULL array should fail");
  result = arr_remove_range(null_arr, 0, 1);
  REQUIRE_WITH_MSG(result == OUT_OF_BOUNDS, "Remove from NULL array should fail");
  // Test 9: Large range removal
  int* large_arr = NULL;
  for (int i = 0; i < 10; i++) {
    arr_push(large_arr, i);
  }
  REQUIRE_WITH_MSG(arr_len(large_arr) == 10, "Large array should have 10 elements");
  // Remove middle chunk [3, 7) -> removes 3, 4, 5, 6
  result = arr_remove_range(large_arr, 3, 7);
  REQUIRE_WITH_MSG(result == OK, "Large range removal should succeed");
  REQUIRE_WITH_MSG(arr_len(large_arr) == 6, "Array should have 6 elements remaining");
  // Verify elements: should be [0, 1, 2, 7, 8, 9]
  int expected[] = {0, 1, 2, 7, 8, 9};
  for (size_t i = 0; i < 6; i++) {
    REQUIRE_WITH_MSG(large_arr[i] == expected[i],
                     "Large range removal elements should be correct");
  }
  arr_free(large_arr);
}

void test_arr_pop(void)
{
  double* arr = NULL;
  double  value;
  // Test pop from empty array (should fail)
  Status result = arr_pop(arr, &value);
  REQUIRE_WITH_MSG(result == OUT_OF_BOUNDS, "Pop from empty array should fail");
  // Test pop from NULL array (should fail)
  double* null_arr = NULL;
  result           = arr_pop(null_arr, &value);
  REQUIRE_WITH_MSG(result == OUT_OF_BOUNDS, "Pop from NULL array should fail");
  // Setup array with known values: [10.0, 20.0, 30.0]
  REQUIRE_WITH_MSG(arr_push(arr, 10.0) == OK, "Push should succeed");
  REQUIRE_WITH_MSG(arr_push(arr, 20.0) == OK, "Push should succeed");
  REQUIRE_WITH_MSG(arr_push(arr, 30.0) == OK, "Push should succeed");
  REQUIRE_WITH_MSG(arr_len(arr) == 3, "Array should have 3 elements");
  // Test pop from array with multiple elements
  result = arr_pop(arr, &value);
  REQUIRE_WITH_MSG(result == OK, "Pop should succeed");
  REQUIRE_WITH_MSG(value == 30.0, "Popped value should be 30.0 (last element)");
  REQUIRE_WITH_MSG(arr_len(arr) == 2, "Array should have 2 elements after pop");
  REQUIRE_WITH_MSG(arr[0] == 10.0 && arr[1] == 20.0,
                   "Remaining elements should be correct");
  // Test sequential pops
  result = arr_pop(arr, &value);
  REQUIRE_WITH_MSG(result == OK, "Second pop should succeed");
  REQUIRE_WITH_MSG(value == 20.0, "Popped value should be 20.0");
  REQUIRE_WITH_MSG(arr_len(arr) == 1, "Array should have 1 element after second pop");
  REQUIRE_WITH_MSG(arr[0] == 10.0, "Remaining element should be 10.0");
  // Test pop from single-element array
  result = arr_pop(arr, &value);
  REQUIRE_WITH_MSG(result == OK, "Pop from single element should succeed");
  REQUIRE_WITH_MSG(value == 10.0, "Popped value should be 10.0");
  REQUIRE_WITH_MSG(arr_len(arr) == 0, "Array should be empty after popping last element");
  // Test pop from now-empty array (should fail)
  result = arr_pop(arr, &value);
  REQUIRE_WITH_MSG(result == OUT_OF_BOUNDS, "Pop from empty array should fail");
  arr_free(arr);
  // Test with different data types
  int* int_arr = NULL;
  int  int_value;
  REQUIRE_WITH_MSG(arr_push(int_arr, 42) == OK, "Int push should succeed");
  REQUIRE_WITH_MSG(arr_push(int_arr, 99) == OK, "Int push should succeed");
  result = arr_pop(int_arr, &int_value);
  REQUIRE_WITH_MSG(result == OK, "Int pop should succeed");
  REQUIRE_WITH_MSG(int_value == 99, "Popped int value should be 99");
  REQUIRE_WITH_MSG(arr_len(int_arr) == 1, "Int array should have 1 element left");
  arr_free(int_arr);
  // Test with pointers
  const char*  strings[] = {"first", "second", "third"};
  const char** str_arr   = NULL;
  const char*  str_value;
  REQUIRE_WITH_MSG(arr_push(str_arr, strings[0]) == OK, "String push should succeed");
  REQUIRE_WITH_MSG(arr_push(str_arr, strings[1]) == OK, "String push should succeed");
  REQUIRE_WITH_MSG(arr_push(str_arr, strings[2]) == OK, "String push should succeed");
  result = arr_pop(str_arr, &str_value);
  REQUIRE_WITH_MSG(result == OK, "String pop should succeed");
  REQUIRE_WITH_MSG(str_value == strings[2], "Popped string should be 'third'");
  REQUIRE_WITH_MSG(arr_len(str_arr) == 2, "String array should have 2 elements left");
  arr_free(str_arr);
  // Test capacity behavior - capacity should not decrease on pop
  double* cap_arr = NULL;
  REQUIRE(OK == arr_reserve(cap_arr, 10));
  size_t initial_capacity = _arr_capacity(cap_arr);
  // Fill with some elements
  for (int i = 0; i < 5; i++) {
    arr_push(cap_arr, (double)i);
  }
  // Pop all elements
  for (int i = 0; i < 5; i++) {
    arr_pop(cap_arr, &value);
  }
  REQUIRE_WITH_MSG(arr_len(cap_arr) == 0, "Array should be empty");
  REQUIRE_WITH_MSG(_arr_capacity(cap_arr) == initial_capacity,
                   "Capacity should not decrease");
  arr_free(cap_arr);
  // Test push after pop (ensure array is still usable)
  double* reuse_arr = NULL;
  arr_push(reuse_arr, 1.0);
  arr_push(reuse_arr, 2.0);
  arr_pop(reuse_arr, &value);
  REQUIRE_WITH_MSG(value == 2.0, "Popped value should be 2.0");
  arr_push(reuse_arr, 3.0);
  REQUIRE_WITH_MSG(arr_len(reuse_arr) == 2, "Array should have 2 elements");
  REQUIRE_WITH_MSG(reuse_arr[0] == 1.0, "First element should be 1.0");
  REQUIRE_WITH_MSG(reuse_arr[1] == 3.0, "Second element should be 3.0");
  arr_free(reuse_arr);
}

void test_arr_fibonacci(void)
{
  uint32_t* fibo = NULL;
  REQUIRE(arr_push(fibo, 1) == OK);
  REQUIRE(arr_push(fibo, 1) == OK);
  for (size_t i = 0; i < 10; ++i) {
    size_t const len = arr_len(fibo);
    REQUIRE(arr_push(fibo, fibo[len - 2] + fibo[len - 1]) == OK);
  }
  REQUIRE(arr_len(fibo) == 12);
  uint32_t const expected[12] = {1, 1, 2, 3, 5, 8, 13, 21, 34, 55, 89, 144};
  for (size_t i = 0; i < 12; ++i) {
    REQUIRE(fibo[i] == expected[i]);
  }
  arr_free(fibo);
}

void test_arr_header_alignment(void)
{
  REQUIRE_WITH_MSG(
    (sizeof(_ArrHeader) % sizeof(_MaxAlignCompat)) == 0,
    "Array header must align with the platform's maximum alignment to be compatible "
    "with arbitrary types inside the container. This doesn't guarantee alignment with "
    "SIMD types. The containers are not meant to be used with SIMD types.");
}

// ============================================================================
// String tests
// ============================================================================

void test_str_null_pointer_operations(void)
{
  char* s = NULL;
  REQUIRE_WITH_MSG(str_len(s) == 0, "Null string has length 0");
  REQUIRE_WITH_MSG(str_end(s) == s, "End of NULL string is itself");
  REQUIRE_WITH_MSG(str_remove(s, 0) == OUT_OF_BOUNDS, "Cannot remove from NULL string");
  // Should not crash
  str_free(s);
}

void test_str_push_basic(void)
{
  char* s = NULL;
  REQUIRE(str_push(s, 'h') == OK);
  REQUIRE(str_push(s, 'e') == OK);
  REQUIRE(str_push(s, 'l') == OK);
  REQUIRE(str_push(s, 'l') == OK);
  REQUIRE(str_push(s, 'o') == OK);
  REQUIRE_WITH_MSG(str_len(s) == 5, "String length should be 5");
  REQUIRE_WITH_MSG(strcmp(s, "hello") == 0, "String content should be 'hello'");
  REQUIRE_WITH_MSG(s[str_len(s)] == '\0', "String must be null-terminated");
  str_free(s);
}

void test_str_push_from_null(void)
{
  char* s = NULL;
  // First push allocates
  REQUIRE(str_push(s, 'a') == OK);
  REQUIRE(s != NULL);
  REQUIRE(str_len(s) == 1);
  REQUIRE(s[0] == 'a');
  REQUIRE(s[1] == '\0');
  // Subsequent pushes
  REQUIRE(str_push(s, 'b') == OK);
  REQUIRE(str_len(s) == 2);
  REQUIRE(strcmp(s, "ab") == 0);
  REQUIRE(str_push(s, 'c') == OK);
  REQUIRE(str_len(s) == 3);
  REQUIRE(strcmp(s, "abc") == 0);
  str_free(s);
}

void test_str_remove_basic(void)
{
  char* s = NULL;
  // Build "abcde"
  REQUIRE(str_push(s, 'a') == OK);
  REQUIRE(str_push(s, 'b') == OK);
  REQUIRE(str_push(s, 'c') == OK);
  REQUIRE(str_push(s, 'd') == OK);
  REQUIRE(str_push(s, 'e') == OK);
  // Remove middle character 'c' at index 2
  REQUIRE(str_remove(s, 2) == OK);
  REQUIRE(str_len(s) == 4);
  REQUIRE(strcmp(s, "abde") == 0);
  REQUIRE(s[str_len(s)] == '\0');
  // Remove first character 'a' at index 0
  REQUIRE(str_remove(s, 0) == OK);
  REQUIRE(str_len(s) == 3);
  REQUIRE(strcmp(s, "bde") == 0);
  REQUIRE(s[str_len(s)] == '\0');
  // Remove last character 'e' at index 2
  REQUIRE(str_remove(s, str_len(s) - 1) == OK);
  REQUIRE(str_len(s) == 2);
  REQUIRE(strcmp(s, "bd") == 0);
  REQUIRE(s[str_len(s)] == '\0');
  str_free(s);
}

void test_str_remove_boundary_conditions(void)
{
  // Single character string
  char* s = NULL;
  REQUIRE(str_push(s, 'x') == OK);
  REQUIRE(str_remove(s, 0) == OK);
  REQUIRE_WITH_MSG(str_len(s) == 0, "Empty after removing only character");
  REQUIRE_WITH_MSG(s[0] == '\0', "Still null-terminated when empty");
  // Remove from empty (non-NULL) string
  REQUIRE_WITH_MSG(str_remove(s, 0) == OUT_OF_BOUNDS, "Cannot remove from empty string");
  // One past end
  REQUIRE(str_push(s, 'a') == OK);
  REQUIRE(str_remove(s, str_len(s)) == OUT_OF_BOUNDS);
  // Way past end
  REQUIRE(str_remove(s, str_len(s) + 10) == OUT_OF_BOUNDS);
  // Huge index
  REQUIRE(str_remove(s, SIZE_MAX) == OUT_OF_BOUNDS);
  // Remove from NULL
  char* null_str = NULL;
  REQUIRE(str_remove(null_str, 0) == OUT_OF_BOUNDS);
  str_free(s);
}

void test_str_len_and_end(void)
{
  char* s = NULL;
  REQUIRE(str_len(s) == 0);
  // Build "abc"
  REQUIRE(str_push(s, 'a') == OK);
  REQUIRE(str_push(s, 'b') == OK);
  REQUIRE(str_push(s, 'c') == OK);
  REQUIRE(str_len(s) == 3);
  REQUIRE(str_end(s) == s + str_len(s));
  REQUIRE(*str_end(s) == '\0');
  // After removal
  REQUIRE(str_remove(s, 1) == OK);
  REQUIRE(str_len(s) == 2);
  REQUIRE(str_end(s) == s + str_len(s));
  REQUIRE(*str_end(s) == '\0');
  str_free(s);
}

void test_str_free_and_reuse(void)
{
  char* s = NULL;
  REQUIRE(str_push(s, 'a') == OK);
  REQUIRE(str_push(s, 'b') == OK);
  REQUIRE(str_len(s) == 2);
  str_free(s);
  REQUIRE_WITH_MSG(s == NULL, "Pointer is NULL after free");
  // Reuse after free
  REQUIRE(str_push(s, 'x') == OK);
  REQUIRE(s != NULL);
  REQUIRE(str_len(s) == 1);
  REQUIRE(strcmp(s, "x") == 0);
  str_free(s);
}

void test_str_push_special_characters(void)
{
  char* s = NULL;
  // Whitespace characters
  REQUIRE(str_push(s, ' ') == OK);
  REQUIRE(str_push(s, '\t') == OK);
  REQUIRE(str_push(s, '\n') == OK);
  REQUIRE(str_len(s) == 3);
  REQUIRE(s[0] == ' ');
  REQUIRE(s[1] == '\t');
  REQUIRE(s[2] == '\n');
  REQUIRE(s[3] == '\0');
  str_free(s);
  // Non-ASCII / high bytes
  s = NULL;
  REQUIRE(str_push(s, (char)0xFF) == OK);
  REQUIRE(str_push(s, (char)0x80) == OK);
  REQUIRE(str_push(s, (char)0x01) == OK);
  REQUIRE(str_len(s) == 3);
  REQUIRE(s[0] == (char)0xFF);
  REQUIRE(s[1] == (char)0x80);
  REQUIRE(s[2] == (char)0x01);
  REQUIRE(s[3] == '\0');
  str_free(s);
  // Pushing a null byte - the header count still grows,
  // so str_len reports based on header, not C string length
  s = NULL;
  REQUIRE(str_push(s, 'a') == OK);
  REQUIRE(str_push(s, '\0') == OK);
  REQUIRE(str_push(s, 'b') == OK);
  REQUIRE_WITH_MSG(str_len(s) == 3, "str_len tracks header count, not strlen");
  // But strlen would see only 1
  REQUIRE_WITH_MSG(strlen(s) == 1, "C strlen stops at embedded null");
  str_free(s);
}

void test_str_capacity_growth(void)
{
  char*        s = NULL;
  size_t const n = 256;
  for (size_t i = 0; i < n; i++) {
    char ch = (char)('a' + (char)(i % 26));
    REQUIRE(str_push(s, ch) == OK);
    REQUIRE(str_len(s) == i + 1);
    REQUIRE(s[str_len(s)] == '\0');
    // Capacity must be at least length + 1 (for null terminator)
    REQUIRE(_arr_capacity(s) >= str_len(s) + 1);
  }
  // Verify final content
  for (size_t i = 0; i < n; i++) {
    char expected = (char)('a' + (char)(i % 26));
    REQUIRE(s[i] == expected);
  }
  REQUIRE(str_len(s) == n);
  str_free(s);
}

void test_str_mixed_operations(void)
{
  char* s = NULL;
  // Build "hello"
  REQUIRE(str_push(s, 'h') == OK);
  REQUIRE(str_push(s, 'e') == OK);
  REQUIRE(str_push(s, 'l') == OK);
  REQUIRE(str_push(s, 'l') == OK);
  REQUIRE(str_push(s, 'o') == OK);
  REQUIRE(strcmp(s, "hello") == 0);
  // Remove 'e' at index 1 -> "hllo"
  REQUIRE(str_remove(s, 1) == OK);
  REQUIRE(strcmp(s, "hllo") == 0);
  REQUIRE(s[str_len(s)] == '\0');
  // Push 'e' -> "hlloe"
  REQUIRE(str_push(s, 'e') == OK);
  REQUIRE(strcmp(s, "hlloe") == 0);
  REQUIRE(s[str_len(s)] == '\0');
  // Remove all characters one by one from front
  size_t len = str_len(s);
  for (size_t i = 0; i < len; i++) {
    REQUIRE(str_remove(s, 0) == OK);
  }
  REQUIRE(str_len(s) == 0);
  REQUIRE(s[0] == '\0');
  // Push again after emptying
  REQUIRE(str_push(s, 'z') == OK);
  REQUIRE(str_len(s) == 1);
  REQUIRE(strcmp(s, "z") == 0);
  str_free(s);
}

void test_str_single_character(void)
{
  char* s = NULL;
  REQUIRE(str_push(s, 'x') == OK);
  REQUIRE(str_len(s) == 1);
  REQUIRE(s[0] == 'x');
  REQUIRE(s[1] == '\0');
  REQUIRE(str_end(s) == s + 1);
  // Remove it
  REQUIRE(str_remove(s, 0) == OK);
  REQUIRE(str_len(s) == 0);
  REQUIRE(s[0] == '\0');
  REQUIRE(str_end(s) == s);
  // Push again - recover from empty
  REQUIRE(str_push(s, 'y') == OK);
  REQUIRE(str_len(s) == 1);
  REQUIRE(s[0] == 'y');
  REQUIRE(s[1] == '\0');
  str_free(s);
}

void test_str_long_string(void)
{
  char*        s = NULL;
  size_t const n = 10000;
  // Build a long string
  for (size_t i = 0; i < n; i++) {
    REQUIRE(str_push(s, (char)('A' + (char)(i % 26))) == OK);
  }
  REQUIRE(str_len(s) == n);
  REQUIRE(s[n] == '\0');
  // Verify content
  for (size_t i = 0; i < n; i++) {
    REQUIRE(s[i] == (char)('A' + (char)(i % 26)));
  }
  // Remove 100 characters from the front
  for (size_t i = 0; i < 100; i++) {
    REQUIRE(str_remove(s, 0) == OK);
  }
  REQUIRE(str_len(s) == n - 100);
  REQUIRE(s[str_len(s)] == '\0');
  // Remove from the end
  for (size_t i = 0; i < 100; i++) {
    REQUIRE(str_remove(s, str_len(s) - 1) == OK);
  }
  REQUIRE(str_len(s) == n - 200);
  REQUIRE(s[str_len(s)] == '\0');
  // Remove from middle
  size_t mid = str_len(s) / 2;
  REQUIRE(str_remove(s, mid) == OK);
  REQUIRE(str_len(s) == n - 201);
  REQUIRE(s[str_len(s)] == '\0');
  str_free(s);
}

// str_clear tests

void test_str_clear_basic(void)
{
  char* s = NULL;
  REQUIRE(str_push(s, 'a') == OK);
  REQUIRE(str_push(s, 'b') == OK);
  REQUIRE(str_push(s, 'c') == OK);
  REQUIRE(str_len(s) == 3);
  str_clear(s);
  REQUIRE_WITH_MSG(str_len(s) == 0, "Length is 0 after clear");
  REQUIRE_WITH_MSG(s[0] == '\0', "Null-terminated after clear");
  REQUIRE_WITH_MSG(str_is_empty(s), "String is empty after clear");
  // Capacity should be preserved
  REQUIRE(_arr_capacity(s) >= 1);
  str_free(s);
}

void test_str_clear_null(void)
{
  // Should not crash
  char* s = NULL;
  str_clear(s);
  REQUIRE(s == NULL);
}

void test_str_clear_and_reuse(void)
{
  char* s = NULL;
  REQUIRE(str_push(s, 'x') == OK);
  REQUIRE(str_push(s, 'y') == OK);
  str_clear(s);
  REQUIRE(str_len(s) == 0);
  // Push after clear
  REQUIRE(str_push(s, 'a') == OK);
  REQUIRE(str_len(s) == 1);
  REQUIRE(strcmp(s, "a") == 0);
  // Clear and push again
  str_clear(s);
  REQUIRE(str_push(s, 'b') == OK);
  REQUIRE(str_push(s, 'c') == OK);
  REQUIRE(strcmp(s, "bc") == 0);
  str_free(s);
}

void test_str_clear_already_empty(void)
{
  char* s = NULL;
  REQUIRE(str_push(s, 'a') == OK);
  REQUIRE(str_remove(s, 0) == OK);
  REQUIRE(str_len(s) == 0);
  // Clear an already-empty (but allocated) string
  str_clear(s);
  REQUIRE(str_len(s) == 0);
  REQUIRE(s[0] == '\0');
  str_free(s);
}

// str_push_str tests

void test_str_push_str_basic(void)
{
  char* s = NULL;
  REQUIRE(str_push(s, 'h') == OK);
  REQUIRE(str_push(s, 'i') == OK);
  REQUIRE(str_push_str(s, " world") == OK);
  REQUIRE_WITH_MSG(str_len(s) == 8, "Length after push_str");
  REQUIRE_WITH_MSG(strcmp(s, "hi world") == 0, "Content after push_str");
  REQUIRE(s[str_len(s)] == '\0');
  str_free(s);
}

void test_str_push_str_to_null(void)
{
  // Push string onto NULL pointer
  char* s = NULL;
  REQUIRE(str_push_str(s, "hello") == OK);
  REQUIRE(s != NULL);
  REQUIRE(str_len(s) == 5);
  REQUIRE(strcmp(s, "hello") == 0);
  REQUIRE(s[str_len(s)] == '\0');
  str_free(s);
}

void test_str_push_str_empty_tail(void)
{
  // Push empty string onto existing string
  char* s = NULL;
  REQUIRE(str_push_str(s, "abc") == OK);
  REQUIRE(str_push_str(s, "") == OK);
  REQUIRE_WITH_MSG(str_len(s) == 3, "Length unchanged after pushing empty string");
  REQUIRE(strcmp(s, "abc") == 0);
  str_free(s);
}

void test_str_push_str_empty_tail_to_null(void)
{
  // Push empty string onto NULL - should allocate an empty string, not fail
  char* s = NULL;
  REQUIRE_WITH_MSG(str_push_str(s, "") == OK, "Pushing empty to NULL should succeed");
  REQUIRE(s != NULL);
  REQUIRE(str_len(s) == 0);
  REQUIRE(s[0] == '\0');
  str_free(s);
}

void test_str_push_str_multiple(void)
{
  char* s = NULL;
  REQUIRE(str_push_str(s, "foo") == OK);
  REQUIRE(str_push_str(s, "bar") == OK);
  REQUIRE(str_push_str(s, "baz") == OK);
  REQUIRE(str_len(s) == 9);
  REQUIRE(strcmp(s, "foobarbaz") == 0);
  REQUIRE(s[str_len(s)] == '\0');
  str_free(s);
}

void test_str_push_str_after_remove(void)
{
  char* s = NULL;
  REQUIRE(str_push_str(s, "abcde") == OK);
  // Remove middle character
  REQUIRE(str_remove(s, 2) == OK);
  REQUIRE(strcmp(s, "abde") == 0);
  // Push string after removal
  REQUIRE(str_push_str(s, "XY") == OK);
  REQUIRE(strcmp(s, "abdeXY") == 0);
  REQUIRE(str_len(s) == 6);
  REQUIRE(s[str_len(s)] == '\0');
  str_free(s);
}

void test_str_push_str_after_clear(void)
{
  char* s = NULL;
  REQUIRE(str_push_str(s, "hello") == OK);
  str_clear(s);
  REQUIRE(str_len(s) == 0);
  REQUIRE(str_push_str(s, "world") == OK);
  REQUIRE(strcmp(s, "world") == 0);
  REQUIRE(str_len(s) == 5);
  str_free(s);
}

void test_str_push_str_long(void)
{
  char* s = NULL;
  // Build a long string by appending many times
  for (int i = 0; i < 500; i++) {
    REQUIRE(str_push_str(s, "ab") == OK);
  }
  REQUIRE(str_len(s) == 1000);
  REQUIRE(s[str_len(s)] == '\0');
  // Verify pattern
  for (size_t i = 0; i < 1000; i += 2) {
    REQUIRE(s[i] == 'a');
    REQUIRE(s[i + 1] == 'b');
  }
  str_free(s);
}

void test_str_push_str_single_char(void)
{
  // Push a single-character string (compare behavior with str_push)
  char* s = NULL;
  REQUIRE(str_push_str(s, "x") == OK);
  REQUIRE(str_len(s) == 1);
  REQUIRE(strcmp(s, "x") == 0);
  // Equivalent to str_push
  char* s2 = NULL;
  REQUIRE(str_push(s2, 'x') == OK);
  REQUIRE(str_len(s2) == 1);
  REQUIRE(strcmp(s, s2) == 0);
  str_free(s);
  str_free(s2);
}

// str_is_empty tests

void test_str_is_empty_null(void)
{
  char* s = NULL;
  REQUIRE_WITH_MSG(str_is_empty(s), "NULL string is empty");
}

void test_str_is_empty_after_operations(void)
{
  char* s = NULL;
  REQUIRE(str_is_empty(s));
  REQUIRE(str_push(s, 'a') == OK);
  REQUIRE_WITH_MSG(!str_is_empty(s), "Non-empty after push");
  REQUIRE(str_remove(s, 0) == OK);
  REQUIRE_WITH_MSG(str_is_empty(s), "Empty after removing last char");
  REQUIRE(str_push_str(s, "hi") == OK);
  REQUIRE(!str_is_empty(s));
  str_clear(s);
  REQUIRE_WITH_MSG(str_is_empty(s), "Empty after clear");
  str_free(s);
}

// Mixed operations across new and old API

void test_str_mixed_new_operations(void)
{
  char* s = NULL;
  REQUIRE(str_push_str(s, "hello") == OK);
  REQUIRE(str_push(s, '!') == OK);
  REQUIRE(strcmp(s, "hello!") == 0);
  str_clear(s);
  REQUIRE(str_is_empty(s));
  REQUIRE(str_push(s, 'A') == OK);
  REQUIRE(str_push_str(s, "BC") == OK);
  REQUIRE(strcmp(s, "ABC") == 0);
  REQUIRE(str_remove(s, 1) == OK);
  REQUIRE(strcmp(s, "AC") == 0);
  REQUIRE(str_push_str(s, "DE") == OK);
  REQUIRE(strcmp(s, "ACDE") == 0);
  REQUIRE(str_len(s) == 4);
  REQUIRE(s[str_len(s)] == '\0');
  str_free(s);
}

// String view.

void test_sv_from_str_and_basics(void)
{
  // From a normal string
  char    buf[] = "hello";
  StrView sv    = sv_from_str(buf);
  REQUIRE(sv.start == buf);
  REQUIRE(sv.end == buf + 5);
  REQUIRE(sv_len(sv) == 5);
  REQUIRE(!sv_is_empty(sv));
  // From empty string
  char    empty[] = "";
  StrView e       = sv_from_str(empty);
  REQUIRE(e.start == empty);
  REQUIRE(e.end == empty);
  REQUIRE(sv_len(e) == 0);
  REQUIRE(sv_is_empty(e));
  // From NULL
  StrView n = sv_from_str(NULL);
  REQUIRE(n.start == NULL);
  REQUIRE(n.end == NULL);
  REQUIRE(sv_len(n) == 0);
  REQUIRE(sv_is_empty(n));
}

void test_sv_trim(void)
{
  // Trim left
  char    buf1[] = "  hi";
  StrView sv1    = sv_trim_left(sv_from_str(buf1));
  REQUIRE(sv_len(sv1) == 2);
  REQUIRE(memcmp(sv1.start, "hi", 2) == 0);
  // Trim right
  char    buf2[] = "hi  ";
  StrView sv2    = sv_trim_right(sv_from_str(buf2));
  REQUIRE(sv_len(sv2) == 2);
  REQUIRE(memcmp(sv2.start, "hi", 2) == 0);
  // Trim both
  char    buf3[] = " \t hi \n ";
  StrView sv3    = sv_trim_right(sv_trim_left(sv_from_str(buf3)));
  REQUIRE(sv_len(sv3) == 2);
  REQUIRE(memcmp(sv3.start, "hi", 2) == 0);
  // All whitespace trims to empty
  char    buf4[] = "   ";
  StrView sv4    = sv_trim_left(sv_from_str(buf4));
  REQUIRE(sv_is_empty(sv4));
  char    buf5[] = "   ";
  StrView sv5    = sv_trim_right(sv_from_str(buf5));
  REQUIRE(sv_is_empty(sv5));
  // No whitespace is a no-op
  char    buf6[] = "abc";
  StrView sv6    = sv_trim_left(sv_trim_right(sv_from_str(buf6)));
  REQUIRE(sv_len(sv6) == 3);
  REQUIRE(memcmp(sv6.start, "abc", 3) == 0);
  // Empty view
  StrView sv7 = sv_trim_left(sv_from_str(""));
  REQUIRE(sv_is_empty(sv7));
  StrView sv8 = sv_trim_right(sv_from_str(""));
  REQUIRE(sv_is_empty(sv8));
  // NULL view
  StrView null_sv = sv_trim_left((StrView) {0});
  REQUIRE(null_sv.start == NULL);
  REQUIRE(null_sv.end == NULL);
  null_sv = sv_trim_right((StrView) {0});
  REQUIRE(null_sv.start == NULL);
  REQUIRE(null_sv.end == NULL);
}

void test_sv_split_at_delim(void)
{
  // Basic split on comma
  char    buf[] = "one,two,three";
  StrView sv    = sv_from_str(buf);
  StrView part1 = sv_split_at_delim(&sv, ',');
  REQUIRE(sv_len(part1) == 3);
  REQUIRE(memcmp(part1.start, "one", 3) == 0);
  REQUIRE_WITH_MSG(sv.start == buf + 4, "Remainder starts after delimiter");
  StrView part2 = sv_split_at_delim(&sv, ',');
  REQUIRE(sv_len(part2) == 3);
  REQUIRE(memcmp(part2.start, "two", 3) == 0);
  // Last segment — no more delimiters, returns remainder and nulls out sv
  StrView part3 = sv_split_at_delim(&sv, ',');
  REQUIRE(sv_len(part3) == 5);
  REQUIRE(memcmp(part3.start, "three", 5) == 0);
  REQUIRE(sv.start == NULL);
  REQUIRE(sv.end == NULL);
  // Splitting an exhausted view returns empty
  StrView part4 = sv_split_at_delim(&sv, ',');
  REQUIRE(sv_is_empty(part4));
  // Delimiter at start yields empty first part
  char    buf2[] = ",hello";
  StrView sv2    = sv_from_str(buf2);
  StrView first  = sv_split_at_delim(&sv2, ',');
  REQUIRE(sv_len(first) == 0);
  REQUIRE(sv_len(sv2) == 5);
  REQUIRE(memcmp(sv2.start, "hello", 5) == 0);
  // Delimiter at end yields content then empty
  char    buf3[] = "hello,";
  StrView sv3    = sv_from_str(buf3);
  StrView before = sv_split_at_delim(&sv3, ',');
  REQUIRE(sv_len(before) == 5);
  REQUIRE(memcmp(before.start, "hello", 5) == 0);
  StrView after = sv_split_at_delim(&sv3, ',');
  REQUIRE(sv_len(after) == 0);
  REQUIRE(sv3.start == NULL);
  // No delimiter at all
  char    buf4[] = "none";
  StrView sv4    = sv_from_str(buf4);
  StrView whole  = sv_split_at_delim(&sv4, ',');
  REQUIRE(sv_len(whole) == 4);
  REQUIRE(memcmp(whole.start, "none", 4) == 0);
  REQUIRE(sv4.start == NULL);
}

void test_sv_split_line(void)
{
  char    buf[] = "line1\nline2\nline3";
  StrView sv    = sv_from_str(buf);
  StrView l1    = sv_split_line(&sv);
  REQUIRE(sv_len(l1) == 5);
  REQUIRE(memcmp(l1.start, "line1", 5) == 0);
  StrView l2 = sv_split_line(&sv);
  REQUIRE(sv_len(l2) == 5);
  REQUIRE(memcmp(l2.start, "line2", 5) == 0);
  StrView l3 = sv_split_line(&sv);
  REQUIRE(sv_len(l3) == 5);
  REQUIRE(memcmp(l3.start, "line3", 5) == 0);
  REQUIRE(sv.start == NULL);
}

void test_sv_trim_combined(void)
{
  char    buf1[] = " \t hello \n ";
  StrView sv1    = sv_trim(sv_from_str(buf1));
  REQUIRE(sv_len(sv1) == 5);
  REQUIRE(memcmp(sv1.start, "hello", 5) == 0);
  // No whitespace
  char    buf2[] = "abc";
  StrView sv2    = sv_trim(sv_from_str(buf2));
  REQUIRE(sv_len(sv2) == 3);
  REQUIRE(memcmp(sv2.start, "abc", 3) == 0);
  // All whitespace
  char    buf3[] = "   ";
  StrView sv3    = sv_trim(sv_from_str(buf3));
  REQUIRE(sv_is_empty(sv3));
  // Empty and NULL
  REQUIRE(sv_is_empty(sv_trim(sv_from_str(""))));
  REQUIRE(sv_trim((StrView) {0}).start == NULL);
}

void test_sv_starts_with(void)
{
  char    buf[] = "hello world";
  StrView sv    = sv_from_str(buf);
  REQUIRE(sv_starts_with(sv, "hello"));
  REQUIRE(sv_starts_with(sv, "h"));
  REQUIRE(sv_starts_with(sv, "hello world"));
  REQUIRE(!sv_starts_with(sv, "hello world!"));
  REQUIRE(!sv_starts_with(sv, "world"));
  REQUIRE(!sv_starts_with(sv, "Hello"));
  // NULL prefix
  REQUIRE(!sv_starts_with(sv, NULL));
  // Empty view
  StrView empty = sv_from_str("");
  REQUIRE(!sv_starts_with(empty, "a"));
  // NULL view
  StrView null_sv = (StrView) {0};
  REQUIRE(!sv_starts_with(null_sv, "a"));
  // Single char view
  char    buf2[] = "x";
  StrView sv2    = sv_from_str(buf2);
  REQUIRE(sv_starts_with(sv2, "x"));
  REQUIRE(!sv_starts_with(sv2, "xy"));
}

void test_sv_ends_with(void)
{
  char    buf[] = "hello world";
  StrView sv    = sv_from_str(buf);
  REQUIRE(sv_ends_with(sv, "world"));
  REQUIRE(sv_ends_with(sv, "d"));
  REQUIRE(sv_ends_with(sv, "hello world"));
  REQUIRE(!sv_ends_with(sv, "hello world!"));
  REQUIRE(!sv_ends_with(sv, "hello"));
  REQUIRE(!sv_ends_with(sv, "World"));
  // NULL suffix
  REQUIRE(!sv_ends_with(sv, NULL));
  // Empty view
  StrView empty = sv_from_str("");
  REQUIRE(!sv_ends_with(empty, "a"));
  // NULL view
  StrView null_sv = (StrView) {0};
  REQUIRE(!sv_ends_with(null_sv, "a"));
  // Single char view
  char    buf2[] = "x";
  StrView sv2    = sv_from_str(buf2);
  REQUIRE(sv_ends_with(sv2, "x"));
  REQUIRE(!sv_ends_with(sv2, "yx"));
}

void test_sv_contains_str(void)
{
  char    buf[] = "hello world";
  StrView sv    = sv_from_str(buf);
  REQUIRE(sv_contains_str(sv, "hello"));
  REQUIRE(sv_contains_str(sv, "world"));
  REQUIRE(sv_contains_str(sv, "lo wo"));
  REQUIRE(sv_contains_str(sv, "hello world"));
  REQUIRE(sv_contains_str(sv, "h"));
  REQUIRE(sv_contains_str(sv, "d"));
  REQUIRE(!sv_contains_str(sv, "hello world!"));
  REQUIRE(!sv_contains_str(sv, "xyz"));
  REQUIRE(!sv_contains_str(sv, "Hello"));
  // Repeated first-byte partial matches (regression: infinite loop)
  char    buf2[] = "aaab";
  StrView sv2    = sv_from_str(buf2);
  REQUIRE(sv_contains_str(sv2, "aab"));
  REQUIRE(!sv_contains_str(sv2, "aac"));
  // Empty needle
  REQUIRE(!sv_contains_str(sv, ""));
  // NULL needle
  REQUIRE(!sv_contains_str(sv, NULL));
  // Empty view
  StrView empty = sv_from_str("");
  REQUIRE(!sv_contains_str(empty, "a"));
  // NULL view
  StrView null_sv = (StrView) {0};
  REQUIRE(!sv_contains_str(null_sv, "a"));
  // Single char view
  char    buf3[] = "x";
  StrView sv3    = sv_from_str(buf3);
  REQUIRE(sv_contains_str(sv3, "x"));
  REQUIRE(!sv_contains_str(sv3, "y"));
  REQUIRE(!sv_contains_str(sv3, "xy"));
  // Needle same length as view, no match
  char    buf4[] = "abc";
  StrView sv4    = sv_from_str(buf4);
  REQUIRE(!sv_contains_str(sv4, "abd"));
  REQUIRE(sv_contains_str(sv4, "abc"));
}

void test_sv_find(void)
{
  char    buf[] = "hello";
  StrView sv    = sv_from_str(buf);
  REQUIRE(sv_find(sv, 'h') == buf);
  REQUIRE(sv_find(sv, 'o') == buf + 4);
  REQUIRE(sv_find(sv, 'l') == buf + 2);
  REQUIRE(sv_find(sv, 'z') == NULL);
  // Empty view
  StrView empty = sv_from_str("");
  REQUIRE(sv_find(empty, 'a') == NULL);
  // NULL view
  StrView null_sv = (StrView) {0};
  REQUIRE(sv_find(null_sv, 'a') == NULL);
  // Single char view
  char    buf2[] = "x";
  StrView sv2    = sv_from_str(buf2);
  REQUIRE(sv_find(sv2, 'x') == buf2);
  REQUIRE(sv_find(sv2, 'y') == NULL);
}

void test_sv_rfind(void)
{
  char    buf[] = "hello";
  StrView sv    = sv_from_str(buf);
  // Finds last occurrence
  REQUIRE(sv_rfind(sv, 'l') == buf + 3);
  REQUIRE(sv_rfind(sv, 'h') == buf);
  REQUIRE(sv_rfind(sv, 'o') == buf + 4);
  REQUIRE(sv_rfind(sv, 'z') == NULL);
  // All same characters
  char    buf2[] = "aaaa";
  StrView sv2    = sv_from_str(buf2);
  REQUIRE(sv_rfind(sv2, 'a') == buf2 + 3);
  // Empty view (regression: out-of-bounds dereference)
  StrView empty = sv_from_str("");
  REQUIRE(sv_rfind(empty, 'a') == NULL);
  // NULL view
  StrView null_sv = (StrView) {0};
  REQUIRE(sv_rfind(null_sv, 'a') == NULL);
  // Single char view
  char    buf3[] = "x";
  StrView sv3    = sv_from_str(buf3);
  REQUIRE(sv_rfind(sv3, 'x') == buf3);
  REQUIRE(sv_rfind(sv3, 'y') == NULL);
  // Only first char matches
  char    buf4[] = "abc";
  StrView sv4    = sv_from_str(buf4);
  REQUIRE(sv_rfind(sv4, 'a') == buf4);
  // Only last char matches
  REQUIRE(sv_rfind(sv4, 'c') == buf4 + 2);
}

void test_str_eq(void)
{
  // Equal strings
  char* a = NULL;
  char* b = NULL;
  REQUIRE(str_push_str(a, "hello") == OK);
  REQUIRE(str_push_str(b, "hello") == OK);
  REQUIRE(str_eq(a, b));
  // Different strings, same length
  str_clear(b);
  REQUIRE(str_push_str(b, "world") == OK);
  REQUIRE(!str_eq(a, b));
  // Different lengths
  str_clear(b);
  REQUIRE(str_push_str(b, "hi") == OK);
  REQUIRE(!str_eq(a, b));
  // Both NULL
  REQUIRE(str_eq(NULL, NULL));
  // One NULL
  REQUIRE(!str_eq(a, NULL));
  REQUIRE(!str_eq(NULL, a));
  // Both empty
  str_clear(a);
  str_clear(b);
  REQUIRE(str_eq(a, b));
  str_free(a);
  str_free(b);
}

void test_sv_contains_char(void)
{
  char    buf[] = "hello";
  StrView sv    = sv_from_str(buf);
  REQUIRE(sv_contains_char(sv, 'h'));
  REQUIRE(sv_contains_char(sv, 'o'));
  REQUIRE(!sv_contains_char(sv, 'z'));
  // Empty and NULL views
  REQUIRE(!sv_contains_char(sv_from_str(""), 'a'));
  REQUIRE(!sv_contains_char((StrView) {0}, 'a'));
}

void test_sv_strip_prefix(void)
{
  char    buf[] = "hello world";
  StrView sv    = sv_from_str(buf);
  // Successful strip
  REQUIRE(sv_strip_prefix(&sv, "hello"));
  REQUIRE(sv_len(sv) == 6);
  REQUIRE(memcmp(sv.start, " world", 6) == 0);
  // Strip again on remainder
  REQUIRE(sv_strip_prefix(&sv, " "));
  REQUIRE(sv_len(sv) == 5);
  REQUIRE(memcmp(sv.start, "world", 5) == 0);
  // Prefix not present
  REQUIRE(!sv_strip_prefix(&sv, "xyz"));
  REQUIRE_WITH_MSG(sv_len(sv) == 5, "View unchanged on failed strip");
  // Prefix longer than view
  REQUIRE(!sv_strip_prefix(&sv, "world!!!!"));
  // Strip entire view
  REQUIRE(sv_strip_prefix(&sv, "world"));
  REQUIRE(sv_is_empty(sv));
  // Empty view
  StrView empty = sv_from_str("");
  REQUIRE(!sv_strip_prefix(&empty, "a"));
  // NULL view
  StrView null_sv = (StrView) {0};
  REQUIRE(!sv_strip_prefix(&null_sv, "a"));
  // NULL prefix
  StrView sv2 = sv_from_str(buf);
  REQUIRE(!sv_strip_prefix(&sv2, NULL));
  // NULL pointer to sv
  REQUIRE(!sv_strip_prefix(NULL, "a"));
}

void test_sv_strip_suffix(void)
{
  char    buf[] = "hello world";
  StrView sv    = sv_from_str(buf);
  // Successful strip
  REQUIRE(sv_strip_suffix(&sv, "world"));
  REQUIRE(sv_len(sv) == 6);
  REQUIRE(memcmp(sv.start, "hello ", 6) == 0);
  // Strip again on remainder
  REQUIRE(sv_strip_suffix(&sv, " "));
  REQUIRE(sv_len(sv) == 5);
  REQUIRE(memcmp(sv.start, "hello", 5) == 0);
  // Suffix not present
  REQUIRE(!sv_strip_suffix(&sv, "xyz"));
  REQUIRE_WITH_MSG(sv_len(sv) == 5, "View unchanged on failed strip");
  // Suffix longer than view
  REQUIRE(!sv_strip_suffix(&sv, "!!!!hello"));
  // Strip entire view
  REQUIRE(sv_strip_suffix(&sv, "hello"));
  REQUIRE(sv_is_empty(sv));
  // Empty view
  StrView empty = sv_from_str("");
  REQUIRE(!sv_strip_suffix(&empty, "a"));
  // NULL view
  StrView null_sv = (StrView) {0};
  REQUIRE(!sv_strip_suffix(&null_sv, "a"));
  // NULL suffix
  StrView sv2 = sv_from_str(buf);
  REQUIRE(!sv_strip_suffix(&sv2, NULL));
  // NULL pointer to sv
  REQUIRE(!sv_strip_suffix(NULL, "a"));
}

void test_sv_slice(void)
{
  char    buf[] = "hello world";
  StrView sv    = sv_from_str(buf);
  // Slice from middle
  StrView mid = sv_slice(sv, 2, 7);
  REQUIRE(sv_len(mid) == 5);
  REQUIRE(memcmp(mid.start, "llo w", 5) == 0);
  // Slice from start
  StrView head = sv_slice(sv, 0, 5);
  REQUIRE(sv_len(head) == 5);
  REQUIRE(memcmp(head.start, "hello", 5) == 0);
  // Slice to end
  StrView tail = sv_slice(sv, 6, 11);
  REQUIRE(sv_len(tail) == 5);
  REQUIRE(memcmp(tail.start, "world", 5) == 0);
  // Full slice
  StrView full = sv_slice(sv, 0, 11);
  REQUIRE(sv_len(full) == 11);
  REQUIRE(memcmp(full.start, "hello world", 11) == 0);
  // Empty slice (start == end)
  StrView empty_slice = sv_slice(sv, 3, 3);
  REQUIRE(sv_is_empty(empty_slice));
  REQUIRE(empty_slice.start != NULL);
  // Single char slice
  StrView one = sv_slice(sv, 0, 1);
  REQUIRE(sv_len(one) == 1);
  REQUIRE(*one.start == 'h');
  // Invalid: end > view length
  StrView bad1 = sv_slice(sv, 0, 100);
  REQUIRE(bad1.start == NULL);
  REQUIRE(bad1.end == NULL);
  // Invalid: start > end
  StrView bad2 = sv_slice(sv, 5, 2);
  REQUIRE(bad2.start == NULL);
  REQUIRE(bad2.end == NULL);
  // NULL view
  StrView null_sv = (StrView) {0};
  StrView bad3    = sv_slice(null_sv, 0, 1);
  REQUIRE(bad3.start == NULL);
}

void test_sv_eq(void)
{
  char    buf1[] = "hello";
  char    buf2[] = "hello";
  StrView a      = sv_from_str(buf1);
  StrView b      = sv_from_str(buf2);
  // Equal views (different backing memory)
  REQUIRE(sv_eq(a, b));
  // Same view
  REQUIRE(sv_eq(a, a));
  // Different content, same length
  char    buf3[] = "world";
  StrView c      = sv_from_str(buf3);
  REQUIRE(!sv_eq(a, c));
  // Different lengths
  char    buf4[] = "hi";
  StrView d      = sv_from_str(buf4);
  REQUIRE(!sv_eq(a, d));
  // Both empty
  StrView e1 = sv_from_str("");
  StrView e2 = sv_from_str("");
  REQUIRE(sv_eq(e1, e2));
  // Both NULL
  StrView n1 = (StrView) {0};
  StrView n2 = (StrView) {0};
  REQUIRE(sv_eq(n1, n2));
  // One empty, one NULL (both have len 0)
  REQUIRE(sv_eq(e1, n1));
  // Empty vs non-empty
  REQUIRE(!sv_eq(e1, a));
  // Compare sub-slices
  StrView sub    = sv_slice(a, 0, 3);
  char    buf5[] = "hel";
  StrView match  = sv_from_str(buf5);
  REQUIRE(sv_eq(sub, match));
}

void test_deck_header_alignment(void)
{
  REQUIRE_WITH_MSG(
    (sizeof(_DeckHeader) % sizeof(_MaxAlignCompat)) == 0,
    "Deck header must align with the platform's maximum alignment to be compatible "
    "with arbitrary types inside the container. This doesn't guarantee alignment with "
    "SIMD types. The containers are not meant to be used with SIMD types.");
}

// Helper: build a binary deck of the given depth (port of Rust's binary_deck).
static size_t* _binary_deck(uint8_t depth)
{
  size_t* deck = NULL;
  size_t  n    = (size_t)1 << depth;
  for (size_t i = 0; i < n; ++i) {
    uint8_t tz = (i > 0) ? (uint8_t)__builtin_ctzll(i) : depth;
    if (tz > depth)
      tz = depth;
    REQUIRE(deck_push(deck, i, tz) == OK);
  }
  return deck;
}

void test_deck_basic_push_and_length(void)
{
  size_t* deck = NULL;
  REQUIRE(deck_len(deck) == 0);
  REQUIRE(deck_max_depth(deck) == 0);
  REQUIRE(deck_is_empty(deck));
  REQUIRE(deck_push(deck, ((size_t) {7}), 1) == OK);
  REQUIRE(deck_len(deck) == 1);
  REQUIRE(deck_max_depth(deck) == 1);
  REQUIRE(!deck_is_empty(deck));
  REQUIRE(_deck_header(deck)->item_size == sizeof(size_t));
  REQUIRE(deck[0] == 7);
  REQUIRE(deck_push(deck, ((size_t) {8}), 0) == OK);
  REQUIRE(deck_push(deck, ((size_t) {9}), 0) == OK);
  REQUIRE(deck_len(deck) == 3);
  REQUIRE(_deck_header(deck)->item_size == sizeof(size_t));
  REQUIRE(deck[0] == 7);
  REQUIRE(deck[1] == 8);
  REQUIRE(deck[2] == 9);
  deck_free(deck);
}

void test_deck_binary_deck(void)
{
  uint8_t const DEPTH = 5;
  size_t*       deck  = _binary_deck(DEPTH);
  REQUIRE(deck_len(deck) == (size_t)(1 << DEPTH));
  REQUIRE(deck_max_depth(deck) == DEPTH);
  REQUIRE(_deck_header(deck)->item_size == sizeof(size_t));
  for (size_t i = 0; i < deck_len(deck); ++i) {
    REQUIRE(deck[i] == i);
  }
  deck_free(deck);
}

void test_deck_mark_structure(void)
{
  // Depth-3 binary deck: marks at positions 0,2,4,6 with depths 2,0,1,0.
  size_t*      deck = _binary_deck(3);
  _DeckHeader* h    = _deck_header(deck);
  REQUIRE(h->item_size == sizeof(size_t));
  REQUIRE(arr_len(h->marks) == 4);
  REQUIRE(h->marks[0].depth == 2);
  REQUIRE(h->marks[1].depth == 0);
  REQUIRE(h->marks[2].depth == 1);
  REQUIRE(h->marks[3].depth == 0);
  REQUIRE(h->marks[0].pos == 0);
  REQUIRE(h->marks[1].pos == 2);
  REQUIRE(h->marks[2].pos == 4);
  REQUIRE(h->marks[3].pos == 6);
  deck_free(deck);
}
void test_deck_clear(void)
{
  size_t* deck = _binary_deck(3);
  REQUIRE(deck_len(deck) == 8);
  REQUIRE(_deck_header(deck)->item_size == sizeof(size_t));
  deck_clear(deck);
  REQUIRE(deck_len(deck) == 0);
  REQUIRE(deck_max_depth(deck) == 0);
  REQUIRE(_deck_header(deck)->item_size == sizeof(size_t));
  // Re-use after clear.
  REQUIRE(deck_push(deck, ((size_t) {1}), 1) == OK);
  REQUIRE(deck_push(deck, ((size_t) {2}), 0) == OK);
  REQUIRE(deck_len(deck) == 2);
  REQUIRE(deck_max_depth(deck) == 1);
  REQUIRE(_deck_header(deck)->item_size == sizeof(size_t));
  REQUIRE(deck[0] == 1);
  REQUIRE(deck[1] == 2);
  deck_free(deck);
}

void test_deck_flatten(void)
{
  size_t* deck = _binary_deck(4);
  size_t  n    = deck_len(deck);
  REQUIRE(n == 16);
  REQUIRE(_deck_header(deck)->item_size == sizeof(size_t));
  deck_flatten(deck);
  REQUIRE(deck_max_depth(deck) == 1);
  REQUIRE(_deck_header(deck)->item_size == sizeof(size_t));
  for (size_t i = 0; i < n; ++i) {
    REQUIRE(deck[i] == i);
  }
  _DeckHeader* h = _deck_header(deck);
  REQUIRE(arr_len(h->marks) == 1);
  REQUIRE(h->marks[0].depth == 0);
  REQUIRE(h->marks[0].pos == 0);
  // Flatten is idempotent.
  deck_flatten(deck);
  REQUIRE(deck_max_depth(deck) == 1);
  REQUIRE(arr_len(h->marks) == 1);
  deck_free(deck);
  // Flatten a single-element deck pushed at depth 0: no marks.
  size_t* deck2 = NULL;
  REQUIRE(deck_push(deck2, ((size_t) {5}), 0) == OK);
  REQUIRE(_deck_header(deck2)->item_size == sizeof(size_t));
  deck_flatten(deck2);
  REQUIRE(deck_max_depth(deck2) == 0);
  REQUIRE(_deck_header(deck2)->item_size == sizeof(size_t));
  REQUIRE(arr_len(_deck_header(deck2)->marks) == 0);
  deck_free(deck2);
}

void test_deck_reserve(void)
{
  size_t* deck = NULL;
  REQUIRE(deck_reserve(deck, 32) == OK);
  REQUIRE(deck != NULL);
  REQUIRE(deck_len(deck) == 0);
  REQUIRE(_deck_header(deck)->item_size == sizeof(size_t));
  for (size_t i = 0; i < 32; ++i) {
    REQUIRE(deck_push(deck, i, (i == 0) ? 1 : 0) == OK);
  }
  REQUIRE(deck_len(deck) == 32);
  REQUIRE(_deck_header(deck)->item_size == sizeof(size_t));
  for (size_t i = 0; i < 32; ++i) {
    REQUIRE(deck[i] == i);
  }
  deck_free(deck);
}

void test_deck_depth_clamping(void)
{
  // Depth higher than the first mark's depth should be clamped.
  size_t* deck = NULL;
  REQUIRE(deck_push(deck, ((size_t) {0}), 2) == OK);  // first mark: internal depth 1
  REQUIRE(deck_push(deck, ((size_t) {1}), 0) == OK);
  REQUIRE(deck_push(deck, ((size_t) {2}), 5) == OK);  // should be clamped
  REQUIRE(deck_push(deck, ((size_t) {3}), 0) == OK);
  REQUIRE(deck_len(deck) == 4);
  REQUIRE(deck_max_depth(deck) == 2);
  _DeckHeader* h = _deck_header(deck);
  REQUIRE(h->item_size == sizeof(size_t));
  REQUIRE(arr_len(h->marks) == 2);
  REQUIRE(h->marks[0].depth == 1);
  REQUIRE(h->marks[1].depth <= h->marks[0].depth);
  deck_free(deck);
}

void test_deck_single_element(void)
{
  // Depth 0: bare leaf, no marks.
  size_t* deck = NULL;
  REQUIRE(deck_push(deck, ((size_t) {42}), 0) == OK);
  REQUIRE(deck_len(deck) == 1);
  REQUIRE(deck_max_depth(deck) == 0);
  REQUIRE(_deck_header(deck)->item_size == sizeof(size_t));
  REQUIRE(deck[0] == 42);
  REQUIRE(arr_len(_deck_header(deck)->marks) == 0);
  deck_free(deck);
  // Depth 1.
  size_t* deck2 = NULL;
  REQUIRE(deck_push(deck2, ((size_t) {7}), 1) == OK);
  REQUIRE(deck_len(deck2) == 1);
  REQUIRE(deck_max_depth(deck2) == 1);
  REQUIRE(_deck_header(deck2)->item_size == sizeof(size_t));
  REQUIRE(deck2[0] == 7);
  deck_free(deck2);
}

void test_deck_free_null(void)
{
  size_t* deck = NULL;
  deck_free(deck);
  REQUIRE(deck == NULL);
}

void test_deck_many_pushes(void)
{
  size_t* deck = NULL;
  size_t  n    = 1000;
  for (size_t i = 0; i < n; ++i) {
    REQUIRE(deck_push(deck, i, (i == 0) ? 1 : 0) == OK);
  }
  REQUIRE(deck_len(deck) == n);
  REQUIRE(deck_max_depth(deck) == 1);
  REQUIRE(_deck_header(deck)->item_size == sizeof(size_t));
  for (size_t i = 0; i < n; ++i) {
    REQUIRE(deck[i] == i);
  }
  deck_free(deck);
}

void test_deck_graft(void)
{
  // Graft a depth-3 binary deck: depth should increase by 1, items unchanged.
  size_t* deck = _binary_deck(3);
  REQUIRE(_deck_header(deck)->item_size == sizeof(size_t));
  deck_graft(deck);
  REQUIRE(deck_max_depth(deck) == 4);
  REQUIRE(_deck_header(deck)->item_size == sizeof(size_t));
  for (size_t i = 0; i < 8; ++i) {
    REQUIRE(deck[i] == i);
  }
  // After graft, every original item should have its own depth-0 mark,
  // plus the original marks with depth incremented by 1.
  // Original marks: depths [2,0,1,0] at positions [0,2,4,6].
  // After graft, we expect 8 marks total (one per item position), with:
  //   pos 0: depth 3, pos 1: depth 0, pos 2: depth 1, pos 3: depth 0,
  //   pos 4: depth 2, pos 5: depth 0, pos 6: depth 1, pos 7: depth 0.
  _DeckHeader* h = _deck_header(deck);
  REQUIRE(arr_len(h->marks) == 8);
  uint8_t const expected_depths[] = {3, 0, 1, 0, 2, 0, 1, 0};
  for (size_t i = 0; i < 8; ++i) {
    REQUIRE(h->marks[i].depth == expected_depths[i]);
    REQUIRE(h->marks[i].pos == i);
  }
  deck_free(deck);
  // Graft then flatten roundtrip: items survive.
  size_t* deck2 = _binary_deck(2);
  deck_graft(deck2);
  REQUIRE(_deck_header(deck2)->item_size == sizeof(size_t));
  deck_flatten(deck2);
  REQUIRE(deck_max_depth(deck2) == 1);
  REQUIRE(_deck_header(deck2)->item_size == sizeof(size_t));
  REQUIRE(deck_len(deck2) == 4);
  for (size_t i = 0; i < 4; ++i) {
    REQUIRE(deck2[i] == i);
  }
  deck_free(deck2);
  // Graft a flat (depth-1) deck: each item gets wrapped.
  size_t* deck3 = NULL;
  for (size_t i = 0; i < 3; ++i) {
    REQUIRE(deck_push(deck3, i, (i == 0) ? 1 : 0) == OK);
  }
  REQUIRE(deck_max_depth(deck3) == 1);
  deck_graft(deck3);
  REQUIRE(deck_max_depth(deck3) == 2);
  REQUIRE(_deck_header(deck3)->item_size == sizeof(size_t));
  REQUIRE(deck_len(deck3) == 3);
  // Every item should have its own mark at depth 0 (wrapped individually),
  // plus the original depth-0 mark promoted to depth 1.
  _DeckHeader* h3 = _deck_header(deck3);
  REQUIRE(arr_len(h3->marks) == 3);
  REQUIRE(h3->marks[0].depth == 1);
  REQUIRE(h3->marks[1].depth == 0);
  REQUIRE(h3->marks[2].depth == 0);
  for (size_t i = 0; i < 3; ++i) {
    REQUIRE(h3->marks[i].pos == i);
  }
  deck_free(deck3);
  // Graft an empty deck: should be a no-op.
  size_t* deck4 = NULL;
  deck_graft(deck4);
  REQUIRE(deck_len(deck4) == 0);
}

void test_deck_simplify(void)
{
  // A deck whose mark depths already use every level is unchanged.
  {
    size_t*      deck = _binary_deck(3);
    _DeckHeader* h    = _deck_header(deck);
    REQUIRE(h->item_size == sizeof(size_t));
    size_t const n_marks = arr_len(h->marks);
    uint8_t      depths_before[4];
    for (size_t i = 0; i < n_marks; ++i)
      depths_before[i] = h->marks[i].depth;
    deck_simplify(deck);
    for (size_t i = 0; i < n_marks; ++i) {
      REQUIRE(h->marks[i].depth == depths_before[i]);
    }
    deck_free(deck);
  }
  // A deck with gaps in depth levels: only depths 0 and 4 present.
  // Should be remapped to 0 and 1.
  {
    size_t* deck = NULL;
    REQUIRE(deck_push(deck, ((size_t) {0}), 5) == OK);  // mark depth = 4
    REQUIRE(deck_push(deck, ((size_t) {1}), 0) == OK);
    REQUIRE(deck_push(deck, ((size_t) {2}), 2) == OK);  // mark depth = 1
    REQUIRE(deck_push(deck, ((size_t) {3}), 0) == OK);
    _DeckHeader* h = _deck_header(deck);
    REQUIRE(arr_len(h->marks) == 2);
    // Before simplify: depths are 4 and 1 (clamped from external 5 and 2).
    // Wait — first mark has internal depth 4, second gets clamped to 4.
    // But external 2 -> internal 1, which is <= 4, so no clamping.
    REQUIRE(h->marks[0].depth == 4);
    REQUIRE(h->marks[1].depth == 1);
    deck_simplify(deck);
    REQUIRE(deck_max_depth(deck) == 2);
    REQUIRE(h->item_size == sizeof(size_t));
    REQUIRE(h->marks[0].depth == 1);
    REQUIRE(h->marks[1].depth == 0);
    // Items unchanged.
    for (size_t i = 0; i < 4; ++i) {
      REQUIRE(deck[i] == i);
    }
    deck_free(deck);
  }
  // Simplify is idempotent.
  {
    size_t* deck = NULL;
    REQUIRE(deck_push(deck, ((size_t) {0}), 5) == OK);
    REQUIRE(deck_push(deck, ((size_t) {1}), 0) == OK);
    REQUIRE(deck_push(deck, ((size_t) {2}), 2) == OK);
    REQUIRE(deck_push(deck, ((size_t) {3}), 0) == OK);
    deck_simplify(deck);
    _DeckHeader*  h  = _deck_header(deck);
    uint8_t const d0 = h->marks[0].depth;
    uint8_t const d1 = h->marks[1].depth;
    deck_simplify(deck);
    REQUIRE(h->marks[0].depth == d0);
    REQUIRE(h->marks[1].depth == d1);
    deck_free(deck);
  }
  // Simplify after graft.
  {
    size_t* deck = _binary_deck(3);
    deck_graft(deck);
    REQUIRE(deck_max_depth(deck) == 4);
    // After graft, depths are contiguous (0,1,2,3), so simplify is a no-op.
    _DeckHeader* h             = _deck_header(deck);
    size_t const n_marks       = arr_len(h->marks);
    uint8_t*     depths_before = NULL;
    for (size_t i = 0; i < n_marks; ++i) {
      arr_push(depths_before, h->marks[i].depth);
    }
    deck_simplify(deck);
    REQUIRE(deck_max_depth(deck) == 4);
    for (size_t i = 0; i < n_marks; ++i) {
      REQUIRE(h->marks[i].depth == depths_before[i]);
    }
    arr_free(depths_before);
    deck_free(deck);
  }
  // Simplify on empty/NULL deck is safe.
  {
    size_t* deck = NULL;
    deck_simplify(deck);
    REQUIRE(deck_len(deck) == 0);
  }
}

void _print_size_t(void* item, char* dst, size_t len)
{
  size_t const val = *(size_t*)item;
  snprintf(dst, len, "%zu", val);
}

void test_deck_printf(void)
{
  size_t* deck = _binary_deck(5);
  REQUIRE(_deck_header(deck)->item_size == sizeof(size_t));
  char* output = deck_to_str(deck, _print_size_t);
  REQUIRE(sv_eq(sv_trim(sv_from_str(output)),
                sv_trim(sv_from_str("  5 ---------------| 0\n"
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
  str_free(output);
  // Depth-2 with empty lists.
  DECK_INIT(deck, size_t, ((1, 2, 3), (), (4, 5, 6, 7), (), (8, 9, 10, 11), ()));
  output = deck_to_str(deck, _print_size_t);
  REQUIRE(sv_eq(sv_trim(sv_from_str(output)),
                sv_trim(sv_from_str("  2 ------| 1\n"
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
  str_free(output);
  // Depth-1: Flat list.
  DECK_INIT(deck, size_t, (10, 20, 30));
  output = deck_to_str(deck, _print_size_t);
  REQUIRE(sv_eq(sv_from_str(output),
                sv_from_str("  1 ---| 10\n"
                            "       | 20\n"
                            "       | 30\n")));
  str_free(output);
  // List with single element.
  DECK_INIT(deck, size_t, (42));
  output = deck_to_str(deck, _print_size_t);
  REQUIRE(sv_eq(sv_from_str(output), sv_from_str("  1 ---| 42\n")));
  str_free(output);
  // All empty depth-2.
  DECK_INIT(deck, size_t, ((), (), ()));
  output = deck_to_str(deck, _print_size_t);
  REQUIRE(sv_eq(sv_from_str(output),
                sv_from_str("  2 ------|\n"
                            "     1 ---|\n"
                            "     1 ---|\n")));
  str_free(output);
  // Empty deck.
  deck_clear(deck);
  output = deck_to_str(deck, _print_size_t);
  REQUIRE(output == NULL);
  // Depth-3 with nested empty.
  DECK_INIT(deck, size_t, (((1, 2), ()), (()), ((3))));
  output = deck_to_str(deck, _print_size_t);
  REQUIRE(sv_eq(sv_from_str(output),
                sv_from_str("  3 ---------| 1\n"
                            "             | 2\n"
                            "        1 ---|\n"
                            "     2 ------|\n"
                            "     2 ------| 3\n")));
}

void test_deck_init(void)
{
  size_t*      deck = NULL;
  _DeckHeader* h    = NULL;
  /* depth 1: flat list */
  DECK_INIT(deck, size_t, (10, 20, 30));
  REQUIRE(deck_len(deck) == 3);
  REQUIRE(deck_max_depth(deck) == 1);
  REQUIRE(_deck_header(deck)->item_size == sizeof(size_t));
  REQUIRE(deck[0] == 10);
  REQUIRE(deck[1] == 20);
  REQUIRE(deck[2] == 30);
  h = _deck_header(deck);
  REQUIRE(arr_len(h->marks) == 1);
  REQUIRE(h->marks[0].depth == 0);
  REQUIRE(h->marks[0].pos == 0);
  /* depth 2: re-init clears existing data */
  DECK_INIT(deck, size_t, ((1, 2, 3), (4, 5, 6), (7, 8, 9)));
  REQUIRE(deck_len(deck) == 9);
  REQUIRE(deck_max_depth(deck) == 2);
  REQUIRE(_deck_header(deck)->item_size == sizeof(size_t));
  for (size_t i = 0; i < 9; ++i)
    REQUIRE(deck[i] == i + 1);
  h = _deck_header(deck);
  REQUIRE(arr_len(h->marks) == 3);
  REQUIRE(h->marks[0].depth == 1);
  REQUIRE(h->marks[0].pos == 0);
  REQUIRE(h->marks[1].depth == 0);
  REQUIRE(h->marks[1].pos == 3);
  REQUIRE(h->marks[2].depth == 0);
  REQUIRE(h->marks[2].pos == 6);
  /* depth 3: ruler sequence depths 2,0,1,0 at positions 0,2,4,6 */
  DECK_INIT(deck, size_t, (((1, 2), (3, 4)), ((5, 6), (7, 8))));
  REQUIRE(deck_len(deck) == 8);
  REQUIRE(deck_max_depth(deck) == 3);
  REQUIRE(_deck_header(deck)->item_size == sizeof(size_t));
  for (size_t i = 0; i < 8; ++i)
    REQUIRE(deck[i] == i + 1);
  h = _deck_header(deck);
  REQUIRE(arr_len(h->marks) == 4);
  REQUIRE(h->marks[0].depth == 2);
  REQUIRE(h->marks[0].pos == 0);
  REQUIRE(h->marks[1].depth == 0);
  REQUIRE(h->marks[1].pos == 2);
  REQUIRE(h->marks[2].depth == 1);
  REQUIRE(h->marks[2].pos == 4);
  REQUIRE(h->marks[3].depth == 0);
  REQUIRE(h->marks[3].pos == 6);
  deck_free(deck);
}

// ========== DeckView ==========

void _print_deck_view(DeckView const* v)  // DEBUG
{
  fprintf(stderr,
          "\nDeckView {\n"
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
          (void*)v->items,
          v->n_items,
          v->item_size,
          (void*)v->marks,
          (void*)v->stride_offset,
          v->n_marks,
          (void*)v->strides,
          v->depth,
          v->start,
          v->end);
}

void test_dv_binary_deck(void)
{
  const uint8_t DEPTH = 5;
  size_t*       deck  = _binary_deck(DEPTH);
  REQUIRE(DEPTH == deck_max_depth(deck));
  REQUIRE((1 << DEPTH) == deck_len(deck));
  REQUIRE(_deck_header(deck)->item_size == sizeof(size_t));
  {  // Iterate from level 5.
    size_t   counter = 0;
    DeckView v5      = dv_from_deck(deck, 5);
    do {
      REQUIRE(5 == dv_depth(&v5));
      REQUIRE(32 == dv_len(&v5));
      DeckView v4 = dv_child(&v5);
      do {
        REQUIRE(4 == dv_depth(&v4));
        REQUIRE(16 == dv_len(&v4));
        DeckView v3 = dv_child(&v4);
        do {
          REQUIRE(3 == dv_depth(&v3));
          REQUIRE(8 == dv_len(&v3));
          DeckView v2 = dv_child(&v3);
          do {
            REQUIRE(2 == dv_depth(&v2));
            REQUIRE(4 == dv_len(&v2));
            DeckView v1 = dv_child(&v2);
            do {
              REQUIRE(1 == dv_depth(&v1));
              REQUIRE(2 == dv_len(&v1));
              size_t const* items = dv_item_ptr(&v1);
              REQUIRE(items != NULL);
              size_t const* end = items + dv_len(&v1);
              while (items != end) {
                REQUIRE(*(items++) == counter++);
              }
            } while (dv_advance(&v1));
          } while (dv_advance(&v2));
        } while (dv_advance(&v3));
      } while (dv_advance(&v4));
    } while (dv_advance(&v5));
    REQUIRE(counter == 32);
  }
  {  // Iterate from level 4.
    DeckView v4      = dv_from_deck(deck, 4);
    size_t   counter = 0;
    do {
      REQUIRE(4 == dv_depth(&v4));
      REQUIRE(16 == dv_len(&v4));
      DeckView v3 = dv_child(&v4);
      do {
        REQUIRE(3 == dv_depth(&v3));
        REQUIRE(8 == dv_len(&v3));
        DeckView v2 = dv_child(&v3);
        do {
          REQUIRE(2 == dv_depth(&v2));
          REQUIRE(4 == dv_len(&v2));
          DeckView v1 = dv_child(&v2);
          do {
            REQUIRE(1 == dv_depth(&v1));
            REQUIRE(2 == dv_len(&v1));
            size_t const* items = dv_item_ptr(&v1);
            REQUIRE(items != NULL);
            size_t const* end = items + dv_len(&v1);
            while (items != end) {
              REQUIRE(*(items++) == counter++);
            }
          } while (dv_advance(&v1));
        } while (dv_advance(&v2));
      } while (dv_advance(&v3));
    } while (dv_advance(&v4));
    REQUIRE(counter == 32);
  }
  {  // Iterate from level 3.
    DeckView v3      = dv_from_deck(deck, 3);
    size_t   counter = 0;
    do {
      REQUIRE(3 == dv_depth(&v3));
      REQUIRE(8 == dv_len(&v3));
      DeckView v2 = dv_child(&v3);
      do {
        REQUIRE(2 == dv_depth(&v2));
        REQUIRE(4 == dv_len(&v2));
        DeckView v1 = dv_child(&v2);
        do {
          REQUIRE(1 == dv_depth(&v1));
          REQUIRE(2 == dv_len(&v1));
          DeckView v0 = dv_child(&v1);
          do {
            size_t const* item = dv_item_ptr(&v0);
            REQUIRE(item != NULL);
            REQUIRE(*item == counter++);
          } while (dv_advance(&v0));
        } while (dv_advance(&v1));
      } while (dv_advance(&v2));
    } while (dv_advance(&v3));
    REQUIRE(counter == 32);
  }
}

// ========== DeckWriter ==========

void test_dw_basic_depth2(void)
{
  // Build ((0,1,2),(3,4,5),(6,7,8)) via DeckWriter, then verify via DeckView.
  uint32_t* deck    = NULL;
  uint32_t  counter = 0;
  {
    DeckWriter w = dw_from_deck(deck, 2);
    for (int g = 0; g < 3; ++g) {
      DeckWriter c = dw_child(&w);
      for (int i = 0; i < 3; ++i) {
        REQUIRE(dw_push(&c, counter) == OK);
        counter++;
      }
      REQUIRE(dw_len(&c) == 3);
    }
  }
  REQUIRE(deck_len(deck) == 9);
  REQUIRE(deck_max_depth(deck) == 2);
  REQUIRE(_deck_header(deck)->item_size == sizeof(uint32_t));
  // Read back via DeckView.
  counter     = 0;
  DeckView v2 = dv_from_deck(deck, 2);
  do {
    DeckView v1 = dv_child(&v2);
    do {
      uint32_t const* items = dv_item_ptr(&v1);
      for (size_t i = 0; i < dv_len(&v1); ++i) {
        REQUIRE(items[i] == counter++);
      }
    } while (dv_advance(&v1));
  } while (dv_advance(&v2));
  REQUIRE(counter == 9);
  deck_free(deck);
}

void test_dw_depth3_nested(void)
{
  // Build 3x3x3 tree at depth 3, mirroring Rust t_deck_writer_basic.
  uint32_t* deck    = NULL;
  uint32_t  counter = 0;
  {
    DeckWriter w3 = dw_from_deck(deck, 3);
    REQUIRE(w3.depth == 3);
    for (int a = 0; a < 3; ++a) {
      DeckWriter w2 = dw_child(&w3);
      REQUIRE(w2.depth == 2);
      for (int b = 0; b < 3; ++b) {
        DeckWriter w1 = dw_child(&w2);
        REQUIRE(w1.depth == 1);
        for (int c = 0; c < 3; ++c) {
          REQUIRE(dw_push(&w1, counter) == OK);
          counter++;
        }
        REQUIRE(dw_len(&w1) == 3);
      }
    }
  }
  REQUIRE(deck_len(deck) == 27);
  REQUIRE(deck_max_depth(deck) == 3);
  REQUIRE(_deck_header(deck)->item_size == sizeof(uint32_t));
  // Read back: iterate depth 3 → 2 → 1 → items.
  counter     = 0;
  DeckView v3 = dv_from_deck(deck, 3);
  do {
    DeckView v2 = dv_child(&v3);
    do {
      DeckView v1 = dv_child(&v2);
      do {
        uint32_t const* items = dv_item_ptr(&v1);
        for (size_t i = 0; i < dv_len(&v1); ++i) {
          REQUIRE(items[i] == counter++);
        }
      } while (dv_advance(&v1));
    } while (dv_advance(&v2));
  } while (dv_advance(&v3));
  REQUIRE(counter == 27);
  deck_free(deck);
}

void test_dw_unbalanced_tree(void)
{
  // ((1), (2,3,4), (5,6)) — groups of different sizes.
  uint32_t* deck = NULL;
  {
    DeckWriter w = dw_from_deck(deck, 2);
    {
      DeckWriter c = dw_child(&w);
      uint32_t   v = 1;
      REQUIRE(dw_push(&c, v) == OK);
    }
    {
      DeckWriter c = dw_child(&w);
      uint32_t   v;
      v = 2;
      REQUIRE(dw_push(&c, v) == OK);
      v = 3;
      REQUIRE(dw_push(&c, v) == OK);
      v = 4;
      REQUIRE(dw_push(&c, v) == OK);
    }
    {
      DeckWriter c = dw_child(&w);
      uint32_t   v;
      v = 5;
      REQUIRE(dw_push(&c, v) == OK);
      v = 6;
      REQUIRE(dw_push(&c, v) == OK);
    }
  }
  REQUIRE(deck_len(deck) == 6);
  REQUIRE(_deck_header(deck)->item_size == sizeof(uint32_t));
  // Verify structure.
  DeckView outer = dv_from_deck(deck, 2);
  DeckView g1    = dv_child(&outer);
  REQUIRE(dv_len(&g1) == 1);
  REQUIRE(*(uint32_t*)dv_item_ptr(&g1) == 1);
  dv_advance(&g1);

  // Cannot reuse g1 after advance past end for depth>0,
  // so re-derive from outer after advance.
  dv_advance(&outer);
  // But outer only has one top-level group, so we iterate children instead.
  // Let's just verify sequentially.
  uint32_t expected[] = {1, 2, 3, 4, 5, 6};
  size_t   idx        = 0;
  DeckView top        = dv_from_deck(deck, 2);
  do {
    DeckView inner = dv_child(&top);
    do {
      uint32_t const* items = dv_item_ptr(&inner);
      for (size_t i = 0; i < dv_len(&inner); ++i) {
        REQUIRE(items[i] == expected[idx++]);
      }
    } while (dv_advance(&inner));
  } while (dv_advance(&top));
  REQUIRE(idx == 6);
  deck_free(deck);
}

void test_dw_empty_groups(void)
{
  // Build ((), (1,2), (), (3), ()) via dw_close for empty groups.
  uint32_t* deck = NULL;
  {
    DeckWriter w = dw_from_deck(deck, 2);
    {
      DeckWriter c = dw_child(&w);
      dw_close(&c);  // empty group
    }
    {
      DeckWriter c = dw_child(&w);
      uint32_t   v;
      v = 1;
      REQUIRE(dw_push(&c, v) == OK);
      v = 2;
      REQUIRE(dw_push(&c, v) == OK);
    }
    {
      DeckWriter c = dw_child(&w);
      dw_close(&c);  // empty group
    }
    {
      DeckWriter c = dw_child(&w);
      uint32_t   v = 3;
      REQUIRE(dw_push(&c, v) == OK);
    }
    {
      DeckWriter c = dw_child(&w);
      dw_close(&c);  // empty group
    }
  }
  REQUIRE(deck_len(deck) == 3);
  REQUIRE(_deck_header(deck)->item_size == sizeof(uint32_t));
  REQUIRE(deck[0] == 1);
  REQUIRE(deck[1] == 2);
  REQUIRE(deck[2] == 3);
  // Verify: 5 inner groups, sizes 0,2,0,1,0.
  size_t   group_sizes[] = {0, 2, 0, 1, 0};
  size_t   gi            = 0;
  DeckView top           = dv_from_deck(deck, 2);
  do {
    DeckView inner = dv_child(&top);
    do {
      REQUIRE(dv_len(&inner) == group_sizes[gi++]);
    } while (dv_advance(&inner));
  } while (dv_advance(&top));
  REQUIRE(gi == 5);
  deck_free(deck);
}

void test_dw_nested_empty(void)
{
  // Build (((), (1,2)), ((3,)), (())) at depth 3.
  uint32_t* deck = NULL;
  {
    DeckWriter w3 = dw_from_deck(deck, 3);
    {
      DeckWriter w2 = dw_child(&w3);
      {
        DeckWriter w1 = dw_child(&w2);
        dw_close(&w1);  // empty inner
      }
      {
        DeckWriter w1 = dw_child(&w2);
        uint32_t   v;
        v = 1;
        REQUIRE(dw_push(&w1, v) == OK);
        v = 2;
        REQUIRE(dw_push(&w1, v) == OK);
      }
    }
    {
      DeckWriter w2 = dw_child(&w3);
      {
        DeckWriter w1 = dw_child(&w2);
        uint32_t   v  = 3;
        REQUIRE(dw_push(&w1, v) == OK);
      }
    }
    {
      DeckWriter w2 = dw_child(&w3);
      {
        DeckWriter w1 = dw_child(&w2);
        dw_close(&w1);  // empty inner
      }
    }
  }
  REQUIRE(deck_len(deck) == 3);
  REQUIRE(deck_max_depth(deck) == 3);
  REQUIRE(_deck_header(deck)->item_size == sizeof(uint32_t));
  // Verify structure by iterating depth 3 → 2 → 1.
  // Expected: (((), (1,2)), ((3,)), (()))
  DeckView v3 = dv_from_deck(deck, 3);
  // Only one top-level group.
  DeckView mid = dv_child(&v3);
  // First mid group: ((), (1,2))
  {
    DeckView inner = dv_child(&mid);
    REQUIRE(dv_len(&inner) == 0);  // empty
    REQUIRE(dv_advance(&inner));
    REQUIRE(dv_len(&inner) == 2);
    uint32_t const* items = dv_item_ptr(&inner);
    REQUIRE(items[0] == 1);
    REQUIRE(items[1] == 2);
    REQUIRE(!dv_advance(&inner));
  }
  REQUIRE(dv_advance(&mid));
  // Second mid group: ((3,))
  {
    DeckView inner = dv_child(&mid);
    REQUIRE(dv_len(&inner) == 1);
    REQUIRE(*(uint32_t*)dv_item_ptr(&inner) == 3);
    REQUIRE(!dv_advance(&inner));
  }
  REQUIRE(dv_advance(&mid));
  // Third mid group: (())
  {
    DeckView inner = dv_child(&mid);
    REQUIRE(dv_len(&inner) == 0);
    REQUIRE(!dv_advance(&inner));
  }
  REQUIRE(!dv_advance(&mid));
  deck_free(deck);
}

void test_dw_single_element_deep(void)
{
  // One item wrapped at depth 5: (((((42)))))
  uint32_t* deck = NULL;
  {
    DeckWriter w5 = dw_from_deck(deck, 5);
    DeckWriter w4 = dw_child(&w5);
    DeckWriter w3 = dw_child(&w4);
    DeckWriter w2 = dw_child(&w3);
    DeckWriter w1 = dw_child(&w2);
    uint32_t   v  = 42;
    REQUIRE(dw_push(&w1, v) == OK);
  }
  REQUIRE(deck_len(deck) == 1);
  REQUIRE(deck_max_depth(deck) == 5);
  REQUIRE(_deck_header(deck)->item_size == sizeof(uint32_t));
  REQUIRE(deck[0] == 42);
  // Unwrap all the way down.
  DeckView v5 = dv_from_deck(deck, 5);
  DeckView v4 = dv_child(&v5);
  DeckView v3 = dv_child(&v4);
  DeckView v2 = dv_child(&v3);
  DeckView v1 = dv_child(&v2);
  REQUIRE(dv_len(&v1) == 1);
  REQUIRE(*(uint32_t*)dv_item_ptr(&v1) == 42);
  deck_free(deck);
}

void test_dw_len_tracking(void)
{
  // Verify dw_len reflects items added at each scope level.
  uint32_t* deck = NULL;
  {
    DeckWriter w3 = dw_from_deck(deck, 3);
    REQUIRE(dw_len(&w3) == 0);
    {
      DeckWriter w2 = dw_child(&w3);
      REQUIRE(dw_len(&w2) == 0);
      {
        DeckWriter w1 = dw_child(&w2);
        REQUIRE(dw_len(&w1) == 0);
        uint32_t v = 10;
        dw_push(&w1, v);
        REQUIRE(dw_len(&w1) == 1);
        v = 20;
        dw_push(&w1, v);
        REQUIRE(dw_len(&w1) == 2);
      }
      REQUIRE(dw_len(&w2) == 2);
      {
        DeckWriter w1 = dw_child(&w2);
        uint32_t   v  = 30;
        dw_push(&w1, v);
        REQUIRE(dw_len(&w1) == 1);
      }
      REQUIRE(dw_len(&w2) == 3);
    }
    REQUIRE(dw_len(&w3) == 3);
    {
      DeckWriter w2 = dw_child(&w3);
      REQUIRE(dw_len(&w2) == 0);
      {
        DeckWriter w1 = dw_child(&w2);
        uint32_t   v  = 40;
        dw_push(&w1, v);
      }
      REQUIRE(dw_len(&w2) == 1);
    }
    REQUIRE(dw_len(&w3) == 4);
  }
  REQUIRE(deck_len(deck) == 4);
  REQUIRE(_deck_header(deck)->item_size == sizeof(uint32_t));
  deck_free(deck);
}

void test_dw_append_to_existing(void)
{
  // Build ((1,2),(3,4)) manually, then append (5,6) via writer.
  uint32_t* deck = NULL;
  uint32_t  v;
  v = 1;
  REQUIRE(OK == deck_push(deck, v, 2));
  v = 2;
  REQUIRE(OK == deck_push(deck, v, 0));
  v = 3;
  REQUIRE(OK == deck_push(deck, v, 1));
  v = 4;
  REQUIRE(OK == deck_push(deck, v, 0));
  REQUIRE(deck_len(deck) == 4);
  REQUIRE(_deck_header(deck)->item_size == sizeof(uint32_t));
  // Append another depth-1 group.
  {
    DeckWriter w = dw_from_deck(deck, 1);
    v            = 5;
    dw_push(&w, v);
    v = 6;
    dw_push(&w, v);
  }
  REQUIRE(deck_len(deck) == 6);
  REQUIRE(_deck_header(deck)->item_size == sizeof(uint32_t));
  // Verify: ((1,2),(3,4),(5,6))
  uint32_t counter = 0;
  DeckView top     = dv_from_deck(deck, 2);
  do {
    DeckView inner = dv_child(&top);
    size_t   n     = 0;
    do {
      uint32_t const* items = dv_item_ptr(&inner);
      for (size_t i = 0; i < dv_len(&inner); ++i) {
        REQUIRE(items[i] == ++counter);
      }
      n++;
    } while (dv_advance(&inner));
    REQUIRE(n * 2 <= 6);  // each group has 2 items
  } while (dv_advance(&top));
  REQUIRE(counter == 6);
  deck_free(deck);
}

void test_dw_flat_depth1(void)
{
  // Depth-1 writer: just a flat list.
  uint32_t* deck = NULL;
  {
    DeckWriter w = dw_from_deck(deck, 1);
    for (uint32_t i = 0; i < 5; ++i) {
      uint32_t v = i * 10;
      dw_push(&w, v);
    }
    REQUIRE(dw_len(&w) == 5);
  }
  REQUIRE(deck_len(deck) == 5);
  REQUIRE(deck_max_depth(deck) == 1);
  REQUIRE(_deck_header(deck)->item_size == sizeof(uint32_t));
  for (uint32_t i = 0; i < 5; ++i) {
    REQUIRE(deck[i] == i * 10);
  }
  // Verify via view: one group with 5 items.
  DeckView v1 = dv_from_deck(deck, 1);
  REQUIRE(dv_len(&v1) == 5);
  REQUIRE(!dv_advance(&v1));
  deck_free(deck);
}

void test_dw_deck_item_ptr(void)
{
  // Verify deck_item_ptr points to the right items.
  uint32_t* deck = NULL;
  {
    DeckWriter w = dw_from_deck(deck, 2);
    {
      DeckWriter c = dw_child(&w);
      uint32_t   v;
      v = 100;
      dw_push(&c, v);
      v = 200;
      dw_push(&c, v);
      v = 300;
      dw_push(&c, v);
      uint32_t* items = deck_item_ptr(&c);
      REQUIRE(items[0] == 100);
      REQUIRE(items[1] == 200);
      REQUIRE(items[2] == 300);
      REQUIRE(dw_len(&c) == 3);
    }
    {
      DeckWriter c = dw_child(&w);
      uint32_t   v;
      v = 400;
      dw_push(&c, v);
      v = 500;
      dw_push(&c, v);
      uint32_t* items = deck_item_ptr(&c);
      REQUIRE(items[0] == 400);
      REQUIRE(items[1] == 500);
      REQUIRE(dw_len(&c) == 2);
    }
  }
  REQUIRE(deck_len(deck) == 5);
  REQUIRE(_deck_header(deck)->item_size == sizeof(uint32_t));
  deck_free(deck);
}

void test_dw_close_idempotent(void)
{
  // dw_close on an already-written writer should be a no-op.
  // dw_close on an already-closed writer should be a no-op.
  uint32_t* deck = NULL;
  {
    DeckWriter w = dw_from_deck(deck, 2);
    {
      // Writer that writes — close should be no-op.
      DeckWriter c = dw_child(&w);
      uint32_t   v = 1;
      dw_push(&c, v);
      dw_close(&c);  // has_next_depth already false
    }
    {
      // Writer that is closed twice.
      DeckWriter c = dw_child(&w);
      dw_close(&c);  // creates empty group
      dw_close(&c);  // should be no-op (zeroed)
    }
    {
      DeckWriter c = dw_child(&w);
      uint32_t   v = 2;
      dw_push(&c, v);
    }
  }
  REQUIRE(deck_len(deck) == 2);
  REQUIRE(_deck_header(deck)->item_size == sizeof(uint32_t));
  // Verify: ((1), (), (2))
  size_t   group_sizes[] = {1, 0, 1};
  size_t   gi            = 0;
  DeckView top           = dv_from_deck(deck, 2);
  do {
    DeckView inner = dv_child(&top);
    do {
      REQUIRE(dv_len(&inner) == group_sizes[gi++]);
    } while (dv_advance(&inner));
  } while (dv_advance(&top));
  REQUIRE(gi == 3);
  deck_free(deck);
}

void test_dw_all_empty_depth3(void)
{
  // (((), ()), (())) — depth 3, no items at all.
  uint32_t* deck = NULL;
  {
    DeckWriter w3 = dw_from_deck(deck, 3);
    {
      DeckWriter w2 = dw_child(&w3);
      {
        DeckWriter w1 = dw_child(&w2);
        dw_close(&w1);
      }
      {
        DeckWriter w1 = dw_child(&w2);
        dw_close(&w1);
      }
    }
    {
      DeckWriter w2 = dw_child(&w3);
      {
        DeckWriter w1 = dw_child(&w2);
        dw_close(&w1);
      }
    }
  }
  REQUIRE(deck_len(deck) == 0);
  REQUIRE(deck_max_depth(deck) == 3);
  REQUIRE(_deck_header(deck)->item_size == sizeof(uint32_t));
  // Verify: one top group, two mid groups, inner groups all empty.
  DeckView v3  = dv_from_deck(deck, 3);
  DeckView mid = dv_child(&v3);
  // First mid: 2 empty children.
  {
    DeckView inner = dv_child(&mid);
    REQUIRE(dv_len(&inner) == 0);
    REQUIRE(dv_advance(&inner));
    REQUIRE(dv_len(&inner) == 0);
    REQUIRE(!dv_advance(&inner));
  }
  REQUIRE(dv_advance(&mid));
  // Second mid: 1 empty child.
  {
    DeckView inner = dv_child(&mid);
    REQUIRE(dv_len(&inner) == 0);
    REQUIRE(!dv_advance(&inner));
  }
  REQUIRE(!dv_advance(&mid));
  deck_free(deck);
}

// ========== Dims (Units) ==========

void test_dims_equal(void)
{
  OrcDims a = {1, 0, -2, 0, 0, 0, 0};
  OrcDims b = {1, 0, -2, 0, 0, 0, 0};
  OrcDims c = {1, 0, -1, 0, 0, 0, 0};
  REQUIRE(dims_equal(a, b));
  REQUIRE(!dims_equal(a, c));
  // Dimensionless
  OrcDims zero_a = {0, 0, 0, 0, 0, 0, 0};
  OrcDims zero_b = {0, 0, 0, 0, 0, 0, 0};
  REQUIRE(dims_equal(zero_a, zero_b));
  // Differ only in last dimension
  OrcDims d = {0, 0, 0, 0, 0, 0, 1};
  REQUIRE(!dims_equal(zero_a, d));
}

void test_dims_multiply(void)
{
  // force * distance = energy
  OrcDims force  = {1, 1, -2, 0, 0, 0, 0};
  OrcDims length = {1, 0, 0, 0, 0, 0, 0};
  OrcDims out;
  dims_multiply(force, length, out);
  OrcDims energy = {2, 1, -2, 0, 0, 0, 0};
  REQUIRE(dims_equal(out, energy));
  // Multiply by dimensionless is identity
  OrcDims zero = {0, 0, 0, 0, 0, 0, 0};
  dims_multiply(force, zero, out);
  REQUIRE(dims_equal(out, force));
  // Negative exponents cancel
  OrcDims a = {-1, -1, 3, 0, 0, 0, 0};
  OrcDims b = {1, 1, -3, 0, 0, 0, 0};
  dims_multiply(a, b, out);
  REQUIRE(dims_equal(out, zero));
}

void test_dims_divide(void)
{
  // velocity / time = acceleration
  OrcDims velocity = {1, 0, -1, 0, 0, 0, 0};
  OrcDims time     = {0, 0, 1, 0, 0, 0, 0};
  OrcDims out;
  dims_divide(velocity, time, out);
  OrcDims accel = {1, 0, -2, 0, 0, 0, 0};
  REQUIRE(dims_equal(out, accel));
  // Divide by self = dimensionless
  dims_divide(velocity, velocity, out);
  OrcDims zero = {0, 0, 0, 0, 0, 0, 0};
  REQUIRE(dims_equal(out, zero));
  // Divide dimensionless by something = negated exponents
  dims_divide(zero, time, out);
  OrcDims inv_time = {0, 0, -1, 0, 0, 0, 0};
  REQUIRE(dims_equal(out, inv_time));
}

void test_dims_pow(void)
{
  OrcDims length = {1, 0, 0, 0, 0, 0, 0};
  OrcDims out;
  // length^2 = area
  dims_pow(length, 2, out);
  OrcDims area = {2, 0, 0, 0, 0, 0, 0};
  REQUIRE(dims_equal(out, area));
  // length^3 = volume
  dims_pow(length, 3, out);
  OrcDims volume = {3, 0, 0, 0, 0, 0, 0};
  REQUIRE(dims_equal(out, volume));
  // pow 0 = dimensionless
  OrcDims velocity = {1, 0, -1, 0, 0, 0, 0};
  dims_pow(velocity, 0, out);
  OrcDims zero = {0, 0, 0, 0, 0, 0, 0};
  REQUIRE(dims_equal(out, zero));
  // pow 1 = identity
  dims_pow(velocity, 1, out);
  REQUIRE(dims_equal(out, velocity));
  // Negative power
  dims_pow(velocity, -1, out);
  OrcDims inv_vel = {-1, 0, 1, 0, 0, 0, 0};
  REQUIRE(dims_equal(out, inv_vel));
  // pow -2 on multi-dim
  OrcDims force = {1, 1, -2, 0, 0, 0, 0};
  dims_pow(force, -2, out);
  OrcDims expected = {-2, -2, 4, 0, 0, 0, 0};
  REQUIRE(dims_equal(out, expected));
}

// ========== Plugin Functions ==========

void _print_double(void* item, char* dst, size_t len)
{
  double const val = *(double*)item;
  snprintf(dst, len, "%.2f", val);
}

void _print_uint32_t(void* item, char* dst, size_t len)
{
  uint32_t const val = *(uint32_t*)item;
  snprintf(dst, len, "%d", val);
}

// This simulates a function that will lives inside the plugin DLL. It takes a
// list<double>, a uin32_t and outputs the item from the list at that index.
void _plugin_function_list_element(OrcHandle const* list_handle,
                                   OrcHandle const* index_handle,
                                   OrcHandle*       item_handle)
{
  // Check the types of inpuuts.
  REQUIRE(list_handle->type_id.primitive_id == ORC_F64);
  REQUIRE(index_handle->type_id.primitive_id == ORC_U32);
  REQUIRE(item_handle->type_id.primitive_id == ORC_F64);
  // Use the SDK provided combinatorics helper to stride over the input data.
  void* combinations = comb_init((OrcHandle const*[]) {list_handle, index_handle},
                                 (uint8_t const[]) {1, 0},
                                 2,
                                 (OrcHandle*[]) {item_handle},
                                 (uint8_t const[]) {0},
                                 1);
  while (combinations) {  // List processing iterations.
    // Get inputs for the current combination.
    DeckView list_input  = comb_get_input(combinations, 0),
             index_input = comb_get_input(combinations, 1);
    REQUIRE(list_input.depth == 1);
    REQUIRE(index_input.depth == 0);
    // Get output for the current combination.
    DeckWriter* item_ouput = comb_get_output(combinations, 0);
    REQUIRE(item_ouput->depth == 0);
    double* output_ptr = (double*)dw_push_empty(item_ouput);
    {  // This scope simulates the actual doRun of the block.
      double*        list  = (double*)dv_item_ptr(&list_input);
      uint32_t const index = *(uint32_t*)dv_item_ptr(&index_input);
      REQUIRE_WITH_MSG(index < dv_len(&list_input), "Index out of bounds");
      *output_ptr = list[index];  // Copy the output to the writer.
    }
    combinations = comb_advance(combinations);
  }
}

// This simulates a function that takes two F64 scalars and outputs their sum.
void _plugin_function_add_f64(OrcHandle const* a_handle,
                              OrcHandle const* b_handle,
                              OrcHandle*       out_handle)
{
  REQUIRE(a_handle->type_id.primitive_id == ORC_F64);
  REQUIRE(b_handle->type_id.primitive_id == ORC_F64);
  REQUIRE(out_handle->type_id.primitive_id == ORC_F64);
  void* combinations = comb_init((OrcHandle const*[]) {a_handle, b_handle},
                                 (uint8_t const[]) {0, 0},
                                 2,
                                 (OrcHandle*[]) {out_handle},
                                 (uint8_t const[]) {0},
                                 1);
  while (combinations) {
    DeckView a_input = comb_get_input(combinations, 0),
             b_input = comb_get_input(combinations, 1);
    REQUIRE(a_input.depth == 0);
    REQUIRE(b_input.depth == 0);
    DeckWriter* out = comb_get_output(combinations, 0);
    REQUIRE(out->depth == 0);
    double* output_ptr = (double*)dw_push_empty(out);
    {
      double const a = *(double*)dv_item_ptr(&a_input);
      double const b = *(double*)dv_item_ptr(&b_input);
      *output_ptr    = a + b;
    }
    combinations = comb_advance(combinations);
  }
}

// This simulates a function that takes one F64 scalar and outputs its square and cube.
void _plugin_function_sq_cb(OrcHandle const* in_handle,
                            OrcHandle*       out_sq_handle,
                            OrcHandle*       out_cb_handle)
{
  REQUIRE(in_handle->type_id.primitive_id == ORC_F64);
  REQUIRE(out_sq_handle->type_id.primitive_id == ORC_F64);
  REQUIRE(out_cb_handle->type_id.primitive_id == ORC_F64);
  void* combinations = comb_init((OrcHandle const*[]) {in_handle},
                                 (uint8_t const[]) {0},
                                 1,
                                 (OrcHandle*[]) {out_sq_handle, out_cb_handle},
                                 (uint8_t const[]) {0, 0},
                                 2);
  while (combinations) {
    DeckView in_input = comb_get_input(combinations, 0);
    REQUIRE(in_input.depth == 0);
    DeckWriter* out_sq = comb_get_output(combinations, 0);
    DeckWriter* out_cb = comb_get_output(combinations, 1);
    REQUIRE(out_sq->depth == 0);
    REQUIRE(out_cb->depth == 0);
    double* sq_ptr = (double*)dw_push_empty(out_sq);
    double* cb_ptr = (double*)dw_push_empty(out_cb);
    {
      double const x = *(double*)dv_item_ptr(&in_input);
      *sq_ptr        = x * x;
      *cb_ptr        = x * x * x;
    }
    combinations = comb_advance(combinations);
  }
}

// This simulates a function that takes two F64 scalars and outputs their sum and product.
void _plugin_function_add_mul(OrcHandle const* a_handle,
                              OrcHandle const* b_handle,
                              OrcHandle*       out_sum_handle,
                              OrcHandle*       out_prod_handle)
{
  REQUIRE(a_handle->type_id.primitive_id == ORC_F64);
  REQUIRE(b_handle->type_id.primitive_id == ORC_F64);
  REQUIRE(out_sum_handle->type_id.primitive_id == ORC_F64);
  REQUIRE(out_prod_handle->type_id.primitive_id == ORC_F64);
  void* combinations = comb_init((OrcHandle const*[]) {a_handle, b_handle},
                                 (uint8_t const[]) {0, 0},
                                 2,
                                 (OrcHandle*[]) {out_sum_handle, out_prod_handle},
                                 (uint8_t const[]) {0, 0},
                                 2);
  while (combinations) {
    DeckView a_input = comb_get_input(combinations, 0),
             b_input = comb_get_input(combinations, 1);
    REQUIRE(a_input.depth == 0);
    REQUIRE(b_input.depth == 0);
    DeckWriter* out_sum  = comb_get_output(combinations, 0);
    DeckWriter* out_prod = comb_get_output(combinations, 1);
    REQUIRE(out_sum->depth == 0);
    REQUIRE(out_prod->depth == 0);
    double* sum_ptr  = (double*)dw_push_empty(out_sum);
    double* prod_ptr = (double*)dw_push_empty(out_prod);
    {
      double const a = *(double*)dv_item_ptr(&a_input);
      double const b = *(double*)dv_item_ptr(&b_input);
      *sum_ptr       = a + b;
      *prod_ptr      = a * b;
    }
    combinations = comb_advance(combinations);
  }
}

// This simulates a function that takes two depth=1 lists of F64 and outputs
// first(a)+first(b).
void _plugin_function_first_add(OrcHandle const* a_handle,
                                OrcHandle const* b_handle,
                                OrcHandle*       out_handle)
{
  REQUIRE(a_handle->type_id.primitive_id == ORC_F64);
  REQUIRE(b_handle->type_id.primitive_id == ORC_F64);
  REQUIRE(out_handle->type_id.primitive_id == ORC_F64);
  void* combinations = comb_init((OrcHandle const*[]) {a_handle, b_handle},
                                 (uint8_t const[]) {1, 1},
                                 2,
                                 (OrcHandle*[]) {out_handle},
                                 (uint8_t const[]) {0},
                                 1);
  while (combinations) {
    DeckView a_input = comb_get_input(combinations, 0),
             b_input = comb_get_input(combinations, 1);
    REQUIRE(a_input.depth == 1);
    REQUIRE(b_input.depth == 1);
    DeckWriter* out = comb_get_output(combinations, 0);
    REQUIRE(out->depth == 0);
    double* output_ptr = (double*)dw_push_empty(out);
    {
      REQUIRE_WITH_MSG(dv_len(&a_input) > 0 && dv_len(&b_input) > 0,
                       "Lists must be non-empty");
      double const a_first = *(double*)dv_item_ptr(&a_input);
      double const b_first = *(double*)dv_item_ptr(&b_input);
      *output_ptr          = a_first + b_first;
    }
    combinations = comb_advance(combinations);
  }
}

void test_list_item_combinations(void)
{
  /*=== This test simulates the running of a list-element block. ===*/
  // Allocate decks - In a real scenario, the host program is allocating these,
  // by calling below functions, defined inside a plugin.
  OrcHandle lists = {0}, indices = {0}, out_items = {0};
  orc_deck_alloc((OrcTypeId) {.primitive_id = ORC_F64, .opaque_id = 0}, &lists);
  orc_deck_alloc((OrcTypeId) {.primitive_id = ORC_U32, .opaque_id = 0}, &indices);
  orc_deck_alloc((OrcTypeId) {.primitive_id = ORC_F64, .opaque_id = 0}, &out_items);
  REQUIRE_WITH_MSG(
    lists.items != NULL && indices.items != NULL && out_items.items != NULL,
    "Unable to allocate decks");
  { /*Depth 2 lists, with one index.*/
    // Populate the inputs with data - in a real scenario this data is computed
    // by upstream functions. Here we pretend.
    DECK_INIT(lists.items,
              double,
              ((1.1, 2.23, 3.34, 3.14159),
               (4.4, 5.5, 6.6, 6.28318),
               (7.7, 8.8, 9.9, 10.1, 3.14159),
               (11.1, 12.1, 13.1, 14.1, 15.1)));
    oh_update(&lists);
    REQUIRE(deck_len(lists.items) == 18);
    DECK_INIT(indices.items, uint32_t, 2);
    oh_update(&indices);
    REQUIRE(deck_len(indices.items) == 1);
    // Run the block - In a real scenario, this function is provided by a plugin DLL.
    _plugin_function_list_element(&lists, &indices, &out_items);
    // Check the outputs. The input had 4 lists, so the output should have 4 items.
    oh_update(&out_items);
    size_t const count = deck_len(out_items.items);
    REQUIRE(count == 4);
    REQUIRE(deck_max_depth(out_items.items) == 1);
    // Output should contain the #2 item from every input list.
    double const  expected[] = {3.34, 6.6, 9.9, 13.1};
    double* const actual     = (double*)out_items.items;
    for (size_t i = 0; i < count; ++i) {
      REQUIRE(actual[i] == expected[i]);
    }
  }
  { /* Same depth 2 lists, with a list of indices. */
    DECK_INIT(indices.items, uint32_t, (0, 1, 2));
    oh_update(&indices);
    deck_clear(out_items.items);  // Clear the outputs.
    oh_update(&out_items);
    // Run the block - In a real scenario, this function is provided by a plugin DLL.
    _plugin_function_list_element(&lists, &indices, &out_items);
    oh_update(&out_items);
    size_t const count = deck_len(out_items.items);
    REQUIRE(count == 4);
    REQUIRE(deck_max_depth(out_items.items) == 1);
    double const  expected[] = {1.1, 5.5, 9.9, 13.1};
    double* const actual     = (double*)out_items.items;
    for (size_t i = 0; i < count; ++i) {
      REQUIRE(actual[i] == expected[i]);
    }
  }
  { /* Same depth 2 lists as before, with depth-2 indices. */
    DECK_INIT(indices.items, uint32_t, ((0, 1, 2), (1, 2, 3)));
    oh_update(&indices);
    deck_clear(out_items.items);  // Clear the outputs.
    oh_update(&out_items);
    // Run the block - In a real scenario, this function is provided by a plugin DLL.
    _plugin_function_list_element(&lists, &indices, &out_items);
    oh_update(&out_items);
    double* actual   = (double*)out_items.items;
    double* expected = NULL;
    DECK_INIT(expected, double, ((1.1, 5.5, 9.9, 13.1), (2.23, 6.6, 10.1, 14.1)));
    REQUIRE(deck_max_depth(actual) == deck_max_depth(expected));
    size_t n_marks = 0;
    {
      _DeckHeader* h = _deck_header(actual);
      n_marks        = arr_len(h->marks);
      REQUIRE(arr_len(_deck_header(expected)->marks) == n_marks);
    }
    for (size_t i = 0; i < n_marks; ++i) {
      OrcMark const m1 = _deck_header(expected)->marks[i];
      OrcMark const m2 = _deck_header(actual)->marks[i];
      REQUIRE(m1.pos == m2.pos && m1.depth == m2.depth);
    }
    size_t const count = deck_len(actual);
    REQUIRE(count == deck_len(expected));
    for (size_t i = 0; i < count; ++i) {
      REQUIRE(actual[i] == expected[i]);
    }
    deck_free(expected);
  }
  // Clean up decks - In a real scenario, the host program is cleaning up, by calling
  // below functions, which are defined inside a plugin.
  orc_deck_free(&lists);
  orc_deck_free(&indices);
  orc_deck_free(&out_items);
}

void test_add_f64_combinations(void)
{
  /*=== Tests two-input scalar addition: equal lengths, broadcast, and depth-2 inputs.
   * ===*/
  OrcHandle a = {0}, b = {0}, out = {0};
  orc_deck_alloc((OrcTypeId) {.primitive_id = ORC_F64, .opaque_id = 0}, &a);
  orc_deck_alloc((OrcTypeId) {.primitive_id = ORC_F64, .opaque_id = 0}, &b);
  orc_deck_alloc((OrcTypeId) {.primitive_id = ORC_F64, .opaque_id = 0}, &out);
  REQUIRE_WITH_MSG(a.items != NULL && b.items != NULL && out.items != NULL,
                   "Unable to allocate decks");

  { /* Flat equal-length inputs: a and b each have 3 scalars (stack_depth=2). */
    DECK_INIT(a.items, double, (1.0, 2.0, 3.0));
    oh_update(&a);
    DECK_INIT(b.items, double, (10.0, 20.0, 30.0));
    oh_update(&b);

    _plugin_function_add_f64(&a, &b, &out);

    oh_update(&out);
    size_t const count = deck_len(out.items);
    REQUIRE(count == 3);
    REQUIRE(deck_max_depth(out.items) == 1);
    double const  expected[] = {11.0, 22.0, 33.0};
    double* const actual     = (double*)out.items;
    for (size_t i = 0; i < count; ++i) {
      REQUIRE(actual[i] == expected[i]);
    }
  }
  { /* Flat inputs, broadcast-last: a has 4 scalars, b has 2 (stack_depth=2). */
    DECK_INIT(a.items, double, (1.0, 2.0, 3.0, 4.0));
    oh_update(&a);
    DECK_INIT(b.items, double, (10.0, 20.0));
    oh_update(&b);
    deck_clear(out.items);
    oh_update(&out);

    _plugin_function_add_f64(&a, &b, &out);

    oh_update(&out);
    size_t const count = deck_len(out.items);
    REQUIRE(count == 4);
    REQUIRE(deck_max_depth(out.items) == 1);
    // b is exhausted at 20.0 and stays there for the remaining elements of a.
    double const  expected[] = {11.0, 22.0, 23.0, 24.0};
    double* const actual     = (double*)out.items;
    for (size_t i = 0; i < count; ++i) {
      REQUIRE(actual[i] == expected[i]);
    }
  }
  { /* Depth-2 inputs, equal groups: 2 inner groups of 2 items each (stack_depth=3). */
    DECK_INIT(a.items, double, ((1.0, 2.0), (3.0, 4.0)));
    oh_update(&a);
    DECK_INIT(b.items, double, ((10.0, 20.0), (30.0, 40.0)));
    oh_update(&b);
    deck_clear(out.items);
    oh_update(&out);

    _plugin_function_add_f64(&a, &b, &out);

    oh_update(&out);
    double* actual   = (double*)out.items;
    double* expected = NULL;
    DECK_INIT(expected, double, ((11.0, 22.0), (33.0, 44.0)));
    REQUIRE(deck_max_depth(actual) == deck_max_depth(expected));
    size_t n_marks = 0;
    {
      _DeckHeader* h = _deck_header(actual);
      n_marks        = arr_len(h->marks);
      REQUIRE(arr_len(_deck_header(expected)->marks) == n_marks);
    }
    for (size_t i = 0; i < n_marks; ++i) {
      OrcMark const m1 = _deck_header(expected)->marks[i];
      OrcMark const m2 = _deck_header(actual)->marks[i];
      REQUIRE(m1.pos == m2.pos && m1.depth == m2.depth);
    }
    size_t const count = deck_len(actual);
    REQUIRE(count == deck_len(expected));
    for (size_t i = 0; i < count; ++i) {
      REQUIRE(actual[i] == expected[i]);
    }
    deck_free(expected);
  }
  { /* Depth-2 inputs, broadcast-last at inner-group level: a has 2 groups, b has 1
       (stack_depth=3). */
    DECK_INIT(a.items, double, ((1.0, 2.0), (3.0, 4.0)));
    oh_update(&a);
    // b is a single depth-2 group (((10.0, 20.0))): b's one group broadcasts across a's
    // two.
    DECK_INIT(b.items, double, (((10.0, 20.0))));
    oh_update(&b);
    deck_clear(out.items);
    oh_update(&out);

    _plugin_function_add_f64(&a, &b, &out);

    oh_update(&out);
    double* actual   = (double*)out.items;
    double* expected = NULL;
    DECK_INIT(expected, double, (((11.0, 22.0), (13.0, 24.0))));
    REQUIRE(deck_max_depth(actual) == deck_max_depth(expected));
    size_t n_marks = 0;
    {
      _DeckHeader* h = _deck_header(actual);
      n_marks        = arr_len(h->marks);
      REQUIRE(arr_len(_deck_header(expected)->marks) == n_marks);
    }
    for (size_t i = 0; i < n_marks; ++i) {
      OrcMark const m1 = _deck_header(expected)->marks[i];
      OrcMark const m2 = _deck_header(actual)->marks[i];
      REQUIRE(m1.pos == m2.pos && m1.depth == m2.depth);
    }
    size_t const count = deck_len(actual);
    REQUIRE(count == deck_len(expected));
    for (size_t i = 0; i < count; ++i) {
      REQUIRE(actual[i] == expected[i]);
    }
    deck_free(expected);
  }
  orc_deck_free(&a);
  orc_deck_free(&b);
  orc_deck_free(&out);
}

// This simulates a function that takes depth=1 lists of F64 and outputs the length of
// each list as U64.
void _plugin_function_list_length(OrcHandle const* in_handle, OrcHandle* out_handle)
{
  REQUIRE(in_handle->type_id.primitive_id == ORC_F64);
  REQUIRE(out_handle->type_id.primitive_id == ORC_U64);
  void* combinations = comb_init((OrcHandle const*[]) {in_handle},
                                 (uint8_t const[]) {1},
                                 1,
                                 (OrcHandle*[]) {out_handle},
                                 (uint8_t const[]) {0},
                                 1);
  while (combinations) {
    DeckView    list_input = comb_get_input(combinations, 0);
    DeckWriter* out        = comb_get_output(combinations, 0);
    REQUIRE(list_input.depth == 1);
    REQUIRE(out->depth == 0);
    uint64_t* output_ptr = (uint64_t*)dw_push_empty(out);
    *output_ptr          = (uint64_t)dv_len(&list_input);
    combinations         = comb_advance(combinations);
  }
}

void test_list_length_combinations(void)
{
  /*=== Tests arg_depth=1 with U64 output: outputs the length of each input list, with
   * empty lists producing zeros. ===*/
  OrcHandle in  = {0};
  OrcHandle out = {0};
  orc_deck_alloc((OrcTypeId) {.primitive_id = ORC_F64, .opaque_id = 0}, &in);
  orc_deck_alloc((OrcTypeId) {.primitive_id = ORC_U64, .opaque_id = 0}, &out);
  REQUIRE_WITH_MSG(in.items != NULL && out.items != NULL, "Unable to allocate decks");

  { /* Depth-2 input: 5 lists, some empty (stack_depth=2). */
    DECK_INIT(in.items, double, ((1.0, 2.0, 3.0), (), (4.0), (), (5.0, 6.0)));
    oh_update(&in);

    _plugin_function_list_length(&in, &out);

    oh_update(&out);
    size_t const count = deck_len(out.items);
    REQUIRE(count == 5);
    REQUIRE(deck_max_depth(out.items) == 1);
    uint64_t const  expected[] = {3, 0, 1, 0, 2};
    uint64_t* const actual     = (uint64_t*)out.items;
    for (size_t i = 0; i < count; ++i) {
      REQUIRE(actual[i] == expected[i]);
    }
  }
  { /* Depth-3 input: two outer groups each containing lists, with empty lists inside
       (stack_depth=3). */
    DECK_INIT(in.items, double, (((1.0, 2.0), ()), ((3.0, 4.0, 5.0), (), (6.0))));
    oh_update(&in);
    deck_clear(out.items);
    oh_update(&out);

    _plugin_function_list_length(&in, &out);

    oh_update(&out);
    uint64_t* actual   = (uint64_t*)out.items;
    uint64_t* expected = NULL;
    DECK_INIT(expected, uint64_t, ((2, 0), (3, 0, 1)));
    REQUIRE(deck_max_depth(actual) == deck_max_depth(expected));
    size_t n_marks = 0;
    {
      _DeckHeader* h = _deck_header(actual);
      n_marks        = arr_len(h->marks);
      REQUIRE(arr_len(_deck_header(expected)->marks) == n_marks);
    }
    for (size_t i = 0; i < n_marks; ++i) {
      OrcMark const m1 = _deck_header(expected)->marks[i];
      OrcMark const m2 = _deck_header(actual)->marks[i];
      REQUIRE(m1.pos == m2.pos && m1.depth == m2.depth);
    }
    size_t const count = deck_len(actual);
    REQUIRE(count == deck_len(expected));
    for (size_t i = 0; i < count; ++i) {
      REQUIRE(actual[i] == expected[i]);
    }
    deck_free(expected);
  }
  orc_deck_free(&in);
  orc_deck_free(&out);
}

void test_two_output_combinations(void)
{
  /*=== Tests multiple-output Combinations: sq+cb (1 in, 2 out) and add+mul (2 in, 2 out).
   * ===*/
  OrcHandle in_a = {0}, in_b = {0}, out1 = {0}, out2 = {0};
  orc_deck_alloc((OrcTypeId) {.primitive_id = ORC_F64, .opaque_id = 0}, &in_a);
  orc_deck_alloc((OrcTypeId) {.primitive_id = ORC_F64, .opaque_id = 0}, &in_b);
  orc_deck_alloc((OrcTypeId) {.primitive_id = ORC_F64, .opaque_id = 0}, &out1);
  orc_deck_alloc((OrcTypeId) {.primitive_id = ORC_F64, .opaque_id = 0}, &out2);
  REQUIRE_WITH_MSG(
    in_a.items != NULL && in_b.items != NULL && out1.items != NULL && out2.items != NULL,
    "Unable to allocate decks");

  { /* One input, two outputs: square and cube of 3 scalars (stack_depth=2). */
    DECK_INIT(in_a.items, double, (2.0, 3.0, 4.0));
    oh_update(&in_a);

    _plugin_function_sq_cb(&in_a, &out1, &out2);

    oh_update(&out1);
    oh_update(&out2);
    REQUIRE(deck_len(out1.items) == 3);
    REQUIRE(deck_len(out2.items) == 3);
    REQUIRE(deck_max_depth(out1.items) == 1);
    REQUIRE(deck_max_depth(out2.items) == 1);
    double const  expected_sq[] = {4.0, 9.0, 16.0};
    double const  expected_cb[] = {8.0, 27.0, 64.0};
    double* const sq_actual     = (double*)out1.items;
    double* const cb_actual     = (double*)out2.items;
    for (size_t i = 0; i < 3; ++i) {
      REQUIRE(sq_actual[i] == expected_sq[i]);
      REQUIRE(cb_actual[i] == expected_cb[i]);
    }
  }
  { /* Two inputs, two outputs: sum and product of 3 scalars each (stack_depth=2). */
    DECK_INIT(in_a.items, double, (1.0, 2.0, 3.0));
    oh_update(&in_a);
    DECK_INIT(in_b.items, double, (4.0, 5.0, 6.0));
    oh_update(&in_b);
    deck_clear(out1.items);
    oh_update(&out1);
    deck_clear(out2.items);
    oh_update(&out2);

    _plugin_function_add_mul(&in_a, &in_b, &out1, &out2);

    oh_update(&out1);
    oh_update(&out2);
    REQUIRE(deck_len(out1.items) == 3);
    REQUIRE(deck_len(out2.items) == 3);
    REQUIRE(deck_max_depth(out1.items) == 1);
    REQUIRE(deck_max_depth(out2.items) == 1);
    double const  expected_sum[]  = {5.0, 7.0, 9.0};
    double const  expected_prod[] = {4.0, 10.0, 18.0};
    double* const sum_actual      = (double*)out1.items;
    double* const prod_actual     = (double*)out2.items;
    for (size_t i = 0; i < 3; ++i) {
      REQUIRE(sum_actual[i] == expected_sum[i]);
      REQUIRE(prod_actual[i] == expected_prod[i]);
    }
  }
  orc_deck_free(&in_a);
  orc_deck_free(&in_b);
  orc_deck_free(&out1);
  orc_deck_free(&out2);
}

void test_first_add_combinations(void)
{
  /*=== Tests arg_depth=1: plugin receives depth-1 list views and sums their first
   * elements. ===*/
  OrcHandle a = {0}, b = {0}, out = {0};
  orc_deck_alloc((OrcTypeId) {.primitive_id = ORC_F64, .opaque_id = 0}, &a);
  orc_deck_alloc((OrcTypeId) {.primitive_id = ORC_F64, .opaque_id = 0}, &b);
  orc_deck_alloc((OrcTypeId) {.primitive_id = ORC_F64, .opaque_id = 0}, &out);
  REQUIRE_WITH_MSG(a.items != NULL && b.items != NULL && out.items != NULL,
                   "Unable to allocate decks");

  { /* Equal-length: a and b each have 3 depth=1 groups (stack_depth=2). */
    DECK_INIT(a.items, double, ((1.0, 99.0), (2.0, 99.0), (3.0, 99.0)));
    oh_update(&a);
    DECK_INIT(b.items, double, ((10.0, 99.0), (20.0, 99.0), (30.0, 99.0)));
    oh_update(&b);

    _plugin_function_first_add(&a, &b, &out);

    oh_update(&out);
    size_t const count = deck_len(out.items);
    REQUIRE(count == 3);
    REQUIRE(deck_max_depth(out.items) == 1);
    double const  expected[] = {11.0, 22.0, 33.0};
    double* const actual     = (double*)out.items;
    for (size_t i = 0; i < count; ++i) {
      REQUIRE(actual[i] == expected[i]);
    }
  }
  { /* Broadcast-last at group level: a has 4 groups, b has 2 (stack_depth=2). */
    DECK_INIT(a.items, double, ((1.0, 99.0), (2.0, 99.0), (3.0, 99.0), (4.0, 99.0)));
    oh_update(&a);
    DECK_INIT(b.items, double, ((10.0, 99.0), (20.0, 99.0)));
    oh_update(&b);
    deck_clear(out.items);
    oh_update(&out);

    _plugin_function_first_add(&a, &b, &out);

    oh_update(&out);
    size_t const count = deck_len(out.items);
    REQUIRE(count == 4);
    REQUIRE(deck_max_depth(out.items) == 1);
    // b is exhausted after its second group and stays at first(b[1])=20.0.
    double const  expected[] = {11.0, 22.0, 23.0, 24.0};
    double* const actual     = (double*)out.items;
    for (size_t i = 0; i < count; ++i) {
      REQUIRE(actual[i] == expected[i]);
    }
  }
  orc_deck_free(&a);
  orc_deck_free(&b);
  orc_deck_free(&out);
}

// ==================== Shuffling decks with a proxy ====================

static OrcHandle _make_flattened_proxy(void* deck)
{
  _DeckHeader*  h     = _deck_header(deck);
  OrcMark*      marks = NULL;
  OrcMark const mark  = (OrcMark) {.depth = 0, .pos = 0};
  arr_push(marks, mark);
  return (OrcHandle) {
    .handle        = 0,
    .items         = NULL,
    .n_items       = 0,
    .item_size     = h->item_size,
    .marks         = marks,
    .stride_offset = NULL,
    .n_marks       = 1,
    .strides       = NULL,
    .type_id       = (OrcTypeId) {.primitive_id = ORC_PROXY, .opaque_id = 0},
    .dims          = {0},
  };
}

static OrcHandle _make_grafted_proxy(void* deck)
{
  _DeckHeader* h         = _deck_header(deck);
  OrcMark*     old_marks = h->marks;
  size_t const n_marks   = arr_len(old_marks);
  OrcMark*     marks     = NULL;
  uint64_t     prev      = 0;
  for (size_t i = 0; i < n_marks; ++i) {
    uint8_t const  new_depth = old_marks[i].depth + 1;
    uint64_t const current   = old_marks[i].pos;
    for (uint64_t j = prev; j < current; ++j) {
      OrcMark const m = {.depth = 0, .pos = j};
      arr_push(marks, m);
    }
    {
      OrcMark const m = {.depth = new_depth, .pos = current};
      arr_push(marks, m);
    }
    prev = current + 1;
  }
  for (uint64_t j = prev; j < (uint64_t)h->count; ++j) {
    OrcMark const m = {.depth = 0, .pos = j};
    arr_push(marks, m);
  }
  return (OrcHandle) {
    .handle        = 0,
    .items         = NULL,
    .n_items       = 0,
    .item_size     = h->item_size,
    .marks         = marks,
    .stride_offset = NULL,
    .n_marks       = (uint64_t)arr_len(marks),
    .strides       = NULL,
    .type_id       = (OrcTypeId) {.primitive_id = ORC_PROXY, .opaque_id = 0},
    .dims          = {0},
  };
}

static OrcHandle _make_simplified_proxy(void* deck)
{
  _DeckHeader* h       = _deck_header(deck);
  size_t const n_marks = arr_len(h->marks);
  OrcMark*     marks   = NULL;
  if (n_marks == 0) {
    return (OrcHandle) {
      .type_id = (OrcTypeId) {.primitive_id = ORC_PROXY, .opaque_id = 0},
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
    arr_push(marks, m);
  }
  return (OrcHandle) {
    .handle        = 0,
    .items         = NULL,
    .n_items       = 0,
    .item_size     = h->item_size,
    .marks         = marks,
    .stride_offset = NULL,
    .n_marks       = (uint64_t)arr_len(marks),
    .strides       = NULL,
    .type_id       = (OrcTypeId) {.primitive_id = ORC_PROXY, .opaque_id = 0},
    .dims          = {0},
  };
}

static OrcHandle _make_shuffle_proxy(OrcItemProxy* pdeck)
{
  OrcHandle handle;
  memset(&handle, 0, sizeof(handle));
  handle.items = pdeck;
  oh_update(&handle);
  handle.type_id = (OrcTypeId) {.primitive_id = ORC_PROXY, .opaque_id = 0};
  return handle;
}

static void _assert_decks_match(void* actual, void* expected, size_t const item_size)
{
  size_t const na = deck_len(actual);
  size_t const ne = deck_len(expected);
  REQUIRE(na == ne);
  REQUIRE(memcmp(actual, expected, na * item_size) == 0);
  REQUIRE(deck_max_depth(actual) == deck_max_depth(expected));
  _DeckHeader* ha  = _deck_header(actual);
  _DeckHeader* he  = _deck_header(expected);
  size_t const nma = arr_len(ha->marks);
  size_t const nme = arr_len(he->marks);
  REQUIRE(nma == nme);
  for (size_t i = 0; i < nma; ++i) {
    REQUIRE(ha->marks[i].pos == he->marks[i].pos);
    REQUIRE(ha->marks[i].depth == he->marks[i].depth);
  }
}

void test_deck_from_proxy_copy_items(void)
{
  /*=== COPY_ITEMS: copies items from input, structure (marks) from proxy. ===*/
  { /* Flatten a depth-2 deck. */
    OrcHandle in = {0}, out = {0};
    orc_deck_alloc((OrcTypeId) {.primitive_id = ORC_F64, .opaque_id = 0}, &in);
    DECK_INIT(in.items, double, ((1.0, 2.0), (3.0, 4.0, 5.0)));
    oh_update(&in);

    OrcHandle proxy = _make_flattened_proxy(in.items);
    orc_deck_from_proxy(&in, 1, ORC_DECK_PROXY_COPY_ITEMS, &proxy, &out);

    REQUIRE(out.type_id.primitive_id == ORC_F64);
    REQUIRE(deck_len(out.items) == 5);
    REQUIRE(deck_max_depth(out.items) == 1);
    double* actual = (double*)out.items;
    REQUIRE(actual[0] == 1.0 && actual[1] == 2.0 && actual[2] == 3.0);
    REQUIRE(actual[3] == 4.0 && actual[4] == 5.0);

    arr_free(proxy.marks);
    orc_deck_free(&out);
    orc_deck_free(&in);
  }
  { /* Flatten a depth-3 deck. */
    OrcHandle in = {0}, out = {0};
    orc_deck_alloc((OrcTypeId) {.primitive_id = ORC_F64, .opaque_id = 0}, &in);
    DECK_INIT(in.items, double, (((1.0, 2.0), (3.0)), ((4.0, 5.0))));
    oh_update(&in);

    OrcHandle proxy = _make_flattened_proxy(in.items);
    orc_deck_from_proxy(&in, 1, ORC_DECK_PROXY_COPY_ITEMS, &proxy, &out);

    REQUIRE(deck_len(out.items) == 5);
    REQUIRE(deck_max_depth(out.items) == 1);
    double* actual = (double*)out.items;
    REQUIRE(actual[0] == 1.0 && actual[4] == 5.0);

    arr_free(proxy.marks);
    orc_deck_free(&out);
    orc_deck_free(&in);
  }
  { /* Graft a flat deck: (1, 2, 3) → ((1), (2), (3)). */
    OrcHandle in = {0}, out = {0};
    orc_deck_alloc((OrcTypeId) {.primitive_id = ORC_F64, .opaque_id = 0}, &in);
    DECK_INIT(in.items, double, (1.0, 2.0, 3.0));
    oh_update(&in);

    OrcHandle proxy = _make_grafted_proxy(in.items);
    orc_deck_from_proxy(&in, 1, ORC_DECK_PROXY_COPY_ITEMS, &proxy, &out);

    double* expected = NULL;
    DECK_INIT(expected, double, ((1.0), (2.0), (3.0)));
    _assert_decks_match(out.items, expected, sizeof(double));
    REQUIRE(out.type_id.primitive_id == ORC_F64);

    deck_free(expected);
    arr_free(proxy.marks);
    orc_deck_free(&out);
    orc_deck_free(&in);
  }
  { /* Graft a depth-2 deck: ((1, 2), (3)) → (((1, 2)), ((3))). */
    OrcHandle in = {0}, out = {0};
    orc_deck_alloc((OrcTypeId) {.primitive_id = ORC_F64, .opaque_id = 0}, &in);
    DECK_INIT(in.items, double, ((1.0, 2.0), (3.0)));
    oh_update(&in);

    OrcHandle proxy = _make_grafted_proxy(in.items);
    orc_deck_from_proxy(&in, 1, ORC_DECK_PROXY_COPY_ITEMS, &proxy, &out);

    double* expected = NULL;
    DECK_INIT(expected, double, (((1.0), (2.0)), ((3.0))));
    _assert_decks_match(out.items, expected, sizeof(double));

    deck_free(expected);
    arr_free(proxy.marks);
    orc_deck_free(&out);
    orc_deck_free(&in);
  }
  { /* Simplify: remove gaps in depth levels. */
    OrcHandle in = {0}, out = {0};
    orc_deck_alloc((OrcTypeId) {.primitive_id = ORC_F64, .opaque_id = 0}, &in);
    DECK_INIT(in.items, double, ((1.0, 2.0), (3.0, 4.0)));
    oh_update(&in);
    /* Graft to create a gap in depth levels, then use simplify proxy. */
    deck_graft(in.items);
    oh_update(&in);

    OrcHandle proxy = _make_simplified_proxy(in.items);
    orc_deck_from_proxy(&in, 1, ORC_DECK_PROXY_COPY_ITEMS, &proxy, &out);

    /* Simplify should match deck_simplify on an equivalent deck. */
    double* expected = NULL;
    DECK_INIT(expected, double, ((1.0, 2.0), (3.0, 4.0)));
    deck_graft(expected);
    deck_simplify(expected);
    _assert_decks_match(out.items, expected, sizeof(double));

    deck_free(expected);
    arr_free(proxy.marks);
    orc_deck_free(&out);
    orc_deck_free(&in);
  }
}

void test_deck_from_proxy_shuffle(void)
{
  /*=== SHUFFLE: copies items one-at-a-time using proxy ItemProxy references. ===*/
  { /* Flat reverse: (1, 2, 3) → (3, 2, 1). */
    OrcHandle in = {0}, out = {0};
    orc_deck_alloc((OrcTypeId) {.primitive_id = ORC_F64, .opaque_id = 0}, &in);
    DECK_INIT(in.items, double, (1.0, 2.0, 3.0));
    oh_update(&in);

    OrcItemProxy* pdeck = NULL;
    REQUIRE(deck_push(pdeck, ((OrcItemProxy) {.tree = 0, .item = 2}), 1) == OK);
    REQUIRE(deck_push(pdeck, ((OrcItemProxy) {.tree = 0, .item = 1}), 0) == OK);
    REQUIRE(deck_push(pdeck, ((OrcItemProxy) {.tree = 0, .item = 0}), 0) == OK);
    OrcHandle proxy = _make_shuffle_proxy(pdeck);
    orc_deck_from_proxy(&in, 1, ORC_DECK_PROXY_SHUFFLE, &proxy, &out);

    double* expected = NULL;
    DECK_INIT(expected, double, (3.0, 2.0, 1.0));
    _assert_decks_match(out.items, expected, sizeof(double));
    REQUIRE(out.type_id.primitive_id == ORC_F64);

    deck_free(expected);
    deck_free(pdeck);
    orc_deck_free(&out);
    orc_deck_free(&in);
  }
  { /* Nested reverse: ((1, 2), (3, 4, 5)) → ((2, 1), (5, 4, 3)). */
    OrcHandle in = {0}, out = {0};
    orc_deck_alloc((OrcTypeId) {.primitive_id = ORC_F64, .opaque_id = 0}, &in);
    DECK_INIT(in.items, double, ((1.0, 2.0), (3.0, 4.0, 5.0)));
    oh_update(&in);

    OrcItemProxy* pdeck = NULL;
    /* First sublist reversed: flat indices 1, 0. */
    REQUIRE(deck_push(pdeck, ((OrcItemProxy) {.tree = 0, .item = 1}), 2) == OK);
    REQUIRE(deck_push(pdeck, ((OrcItemProxy) {.tree = 0, .item = 0}), 0) == OK);
    /* Second sublist reversed: flat indices 4, 3, 2. */
    REQUIRE(deck_push(pdeck, ((OrcItemProxy) {.tree = 0, .item = 4}), 1) == OK);
    REQUIRE(deck_push(pdeck, ((OrcItemProxy) {.tree = 0, .item = 3}), 0) == OK);
    REQUIRE(deck_push(pdeck, ((OrcItemProxy) {.tree = 0, .item = 2}), 0) == OK);
    OrcHandle proxy = _make_shuffle_proxy(pdeck);
    orc_deck_from_proxy(&in, 1, ORC_DECK_PROXY_SHUFFLE, &proxy, &out);

    double* expected = NULL;
    DECK_INIT(expected, double, ((2.0, 1.0), (5.0, 4.0, 3.0)));
    _assert_decks_match(out.items, expected, sizeof(double));

    deck_free(expected);
    deck_free(pdeck);
    orc_deck_free(&out);
    orc_deck_free(&in);
  }
  { /* Generic list_item: pick item at index 1 from each sublist.
       ((1, 2, 3), (4, 5)) → (2, 5). */
    OrcHandle in = {0}, out = {0};
    orc_deck_alloc((OrcTypeId) {.primitive_id = ORC_F64, .opaque_id = 0}, &in);
    DECK_INIT(in.items, double, ((1.0, 2.0, 3.0), (4.0, 5.0)));
    oh_update(&in);

    OrcItemProxy* pdeck = NULL;
    REQUIRE(deck_push(pdeck, ((OrcItemProxy) {.tree = 0, .item = 1}), 1) == OK);
    REQUIRE(deck_push(pdeck, ((OrcItemProxy) {.tree = 0, .item = 4}), 0) == OK);
    OrcHandle proxy = _make_shuffle_proxy(pdeck);
    orc_deck_from_proxy(&in, 1, ORC_DECK_PROXY_SHUFFLE, &proxy, &out);

    REQUIRE(out.type_id.primitive_id == ORC_F64);
    REQUIRE(deck_len(out.items) == 2);
    double* actual = (double*)out.items;
    REQUIRE(actual[0] == 2.0 && actual[1] == 5.0);

    deck_free(pdeck);
    orc_deck_free(&out);
    orc_deck_free(&in);
  }
  { /* Multi-input shuffle: interleave from two decks.
       A=(1, 2), B=(10, 20) → (1, 10, 2, 20). */
    OrcHandle a = {0}, b = {0}, out = {0};
    orc_deck_alloc((OrcTypeId) {.primitive_id = ORC_F64, .opaque_id = 0}, &a);
    orc_deck_alloc((OrcTypeId) {.primitive_id = ORC_F64, .opaque_id = 0}, &b);
    DECK_INIT(a.items, double, (1.0, 2.0));
    oh_update(&a);
    DECK_INIT(b.items, double, (10.0, 20.0));
    oh_update(&b);

    OrcItemProxy* pdeck = NULL;
    REQUIRE(deck_push(pdeck, ((OrcItemProxy) {.tree = 0, .item = 0}), 1) == OK);
    REQUIRE(deck_push(pdeck, ((OrcItemProxy) {.tree = 1, .item = 0}), 0) == OK);
    REQUIRE(deck_push(pdeck, ((OrcItemProxy) {.tree = 0, .item = 1}), 0) == OK);
    REQUIRE(deck_push(pdeck, ((OrcItemProxy) {.tree = 1, .item = 1}), 0) == OK);
    OrcHandle proxy = _make_shuffle_proxy(pdeck);

    OrcHandle inputs[2] = {a, b};
    orc_deck_from_proxy(inputs, 2, ORC_DECK_PROXY_SHUFFLE, &proxy, &out);

    double* expected = NULL;
    DECK_INIT(expected, double, (1.0, 10.0, 2.0, 20.0));
    _assert_decks_match(out.items, expected, sizeof(double));

    deck_free(expected);
    deck_free(pdeck);
    orc_deck_free(&out);
    orc_deck_free(&b);
    orc_deck_free(&a);
  }
}

void test_deck_from_proxy_type_agnostic(void)
{
  /*=== Verifies orc_deck_from_proxy preserves type across u32, i32, i16. ===*/
  { /* u32 flatten. */
    OrcHandle in = {0}, out = {0};
    orc_deck_alloc((OrcTypeId) {.primitive_id = ORC_U32, .opaque_id = 0}, &in);
    DECK_INIT(in.items, uint32_t, ((10, 20), (30)));
    oh_update(&in);

    OrcHandle proxy = _make_flattened_proxy(in.items);
    orc_deck_from_proxy(&in, 1, ORC_DECK_PROXY_COPY_ITEMS, &proxy, &out);

    REQUIRE(out.type_id.primitive_id == ORC_U32);
    REQUIRE(deck_len(out.items) == 3);
    REQUIRE(deck_max_depth(out.items) == 1);
    uint32_t* actual = (uint32_t*)out.items;
    REQUIRE(actual[0] == 10 && actual[1] == 20 && actual[2] == 30);

    arr_free(proxy.marks);
    orc_deck_free(&out);
    orc_deck_free(&in);
  }
  { /* i32 shuffle reverse. */
    OrcHandle in = {0}, out = {0};
    orc_deck_alloc((OrcTypeId) {.primitive_id = ORC_I32, .opaque_id = 0}, &in);
    DECK_INIT(in.items, int32_t, (-1, -2, -3, -4));
    oh_update(&in);

    OrcItemProxy* pdeck = NULL;
    REQUIRE(deck_push(pdeck, ((OrcItemProxy) {.tree = 0, .item = 3}), 1) == OK);
    REQUIRE(deck_push(pdeck, ((OrcItemProxy) {.tree = 0, .item = 2}), 0) == OK);
    REQUIRE(deck_push(pdeck, ((OrcItemProxy) {.tree = 0, .item = 1}), 0) == OK);
    REQUIRE(deck_push(pdeck, ((OrcItemProxy) {.tree = 0, .item = 0}), 0) == OK);
    OrcHandle proxy = _make_shuffle_proxy(pdeck);
    orc_deck_from_proxy(&in, 1, ORC_DECK_PROXY_SHUFFLE, &proxy, &out);

    REQUIRE(out.type_id.primitive_id == ORC_I32);
    REQUIRE(deck_len(out.items) == 4);
    int32_t* actual = (int32_t*)out.items;
    REQUIRE(actual[0] == -4 && actual[1] == -3 && actual[2] == -2 && actual[3] == -1);

    deck_free(pdeck);
    orc_deck_free(&out);
    orc_deck_free(&in);
  }
  { /* i16 graft. */
    OrcHandle in = {0}, out = {0};
    orc_deck_alloc((OrcTypeId) {.primitive_id = ORC_I16, .opaque_id = 0}, &in);
    DECK_INIT(in.items, int16_t, (10, 20, 30));
    oh_update(&in);

    OrcHandle proxy = _make_grafted_proxy(in.items);
    orc_deck_from_proxy(&in, 1, ORC_DECK_PROXY_COPY_ITEMS, &proxy, &out);

    REQUIRE(out.type_id.primitive_id == ORC_I16);
    REQUIRE(deck_len(out.items) == 3);
    REQUIRE(deck_max_depth(out.items) == 2);
    int16_t* actual = (int16_t*)out.items;
    REQUIRE(actual[0] == 10 && actual[1] == 20 && actual[2] == 30);

    arr_free(proxy.marks);
    orc_deck_free(&out);
    orc_deck_free(&in);
  }
}
