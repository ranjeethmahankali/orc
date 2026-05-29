#include "container.h"

#include <limits.h>
#include <math.h>
#include <stddef.h>
#include <stdio.h>
#include <string.h>

#include "macros.h"

#include <inttypes.h>

// Tests

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

void test_que_basic_operations(void)
{
  int* q   = NULL;
  int  val = 0;
  // Test empty queue
  REQUIRE_WITH_MSG(que_is_empty(q) == true, "New queue should be empty");
  REQUIRE_WITH_MSG(que_len(q) == 0, "Empty queue length should be 0");
  // Test push to empty queue
  Status result = que_push(q, 10);
  REQUIRE_WITH_MSG(result == OK, "Push to empty queue should succeed");
  REQUIRE_WITH_MSG(que_is_empty(q) == false, "Queue should not be empty after push");
  REQUIRE_WITH_MSG(que_len(q) == 1, "Queue length should be 1 after push");
  // Test push multiple elements
  REQUIRE_WITH_MSG(que_push(q, 20) == OK, "Second push should succeed");
  REQUIRE_WITH_MSG(que_push(q, 30) == OK, "Third push should succeed");
  REQUIRE_WITH_MSG(que_len(q) == 3, "Queue length should be 3");
  // Test FIFO behavior - first pop should return first pushed (10)
  result = que_pop(q, &val);
  REQUIRE_WITH_MSG(result == OK, "Pop should succeed");
  REQUIRE_WITH_MSG(val == 10, "First pop should return first pushed value (10)");
  REQUIRE_WITH_MSG(que_len(q) == 2, "Queue length should be 2 after pop");
  // Test sequential pops maintain FIFO order
  result = que_pop(q, &val);
  REQUIRE_WITH_MSG(result == OK, "Second pop should succeed");
  REQUIRE_WITH_MSG(val == 20, "Second pop should return 20");
  REQUIRE_WITH_MSG(que_len(q) == 1, "Queue length should be 1");
  result = que_pop(q, &val);
  REQUIRE_WITH_MSG(result == OK, "Third pop should succeed");
  REQUIRE_WITH_MSG(val == 30, "Third pop should return 30");
  REQUIRE_WITH_MSG(que_len(q) == 0, "Queue should be empty after popping all");
  REQUIRE_WITH_MSG(que_is_empty(q) == true, "Queue should report empty");
  // Test push after emptying
  REQUIRE_WITH_MSG(que_push(q, 40) == OK, "Push after emptying should succeed");
  REQUIRE_WITH_MSG(que_len(q) == 1, "Queue length should be 1");
  REQUIRE_WITH_MSG(que_is_empty(q) == false, "Queue should not be empty");
  que_free(q);
}

void test_que_edge_cases(void)
{
  int* q   = NULL;
  int  val = 0;
  // Test operations on NULL queue
  int* null_queue = NULL;
  REQUIRE_WITH_MSG(que_is_empty(null_queue) == true, "NULL queue should be empty");
  REQUIRE_WITH_MSG(que_len(null_queue) == 0, "NULL queue length should be 0");
  REQUIRE_WITH_MSG(que_pop(null_queue, &val) == OUT_OF_BOUNDS,
                   "Pop from NULL should fail");
  // Test pop from empty queue
  REQUIRE_WITH_MSG(que_pop(q, &val) == OUT_OF_BOUNDS, "Pop from empty queue should fail");
  // Add element then test empty pop again
  que_push(q, 100);
  que_pop(q, &val);
  REQUIRE_WITH_MSG(val == 100, "Popped value should be correct");
  REQUIRE_WITH_MSG(que_pop(q, &val) == OUT_OF_BOUNDS,
                   "Pop from newly empty queue should fail");
  // Test single element queue
  que_push(q, 200);
  REQUIRE_WITH_MSG(que_len(q) == 1, "Single element queue length");
  REQUIRE_WITH_MSG(que_is_empty(q) == false, "Single element queue not empty");
  que_pop(q, &val);
  REQUIRE_WITH_MSG(val == 200, "Single element pop value");
  REQUIRE_WITH_MSG(que_is_empty(q) == true, "Queue empty after single pop");
  que_free(q);
}

void test_que_ring_buffer_behavior(void)
{
  int* q   = NULL;
  int  val = 0;
  // Test basic ring buffer behavior - push/pop cycles
  for (int i = 0; i < 5; i++) {
    que_push(q, i * 10);  // [0, 10, 20, 30, 40]
  }
  // Pop first 3 elements to create ring buffer state
  for (int i = 0; i < 3; i++) {
    que_pop(q, &val);
    REQUIRE_WITH_MSG(val == i * 10, "Popped values should be in FIFO order");
  }
  REQUIRE_WITH_MSG(que_len(q) == 2, "Queue should have 2 elements after pops");
  // Queue now has front advanced, elements [30,40] remaining
  _QueueHeader* h = _que_header(q);
  REQUIRE_WITH_MSG(h->front == 3, "Front should be at index 3");
  REQUIRE_WITH_MSG(h->back == 5, "Back should be at index 5");
  // Add more elements to test ring wrapping
  que_push(q, 50);
  que_push(q, 60);
  REQUIRE_WITH_MSG(que_len(q) == 4, "Queue should have 4 elements");
  // Verify elements are still in correct order
  que_pop(q, &val);
  REQUIRE_WITH_MSG(val == 30, "First element should be 30");
  que_pop(q, &val);
  REQUIRE_WITH_MSG(val == 40, "Second element should be 40");
  que_pop(q, &val);
  REQUIRE_WITH_MSG(val == 50, "Third element should be 50");
  que_pop(q, &val);
  REQUIRE_WITH_MSG(val == 60, "Fourth element should be 60");
  REQUIRE_WITH_MSG(que_is_empty(q) == true, "Queue should be empty");
  que_free(q);
}

void test_que_capacity_and_growth(void)
{
  int* q = NULL;
  // Test growth behavior
  const int GROWTH_SIZE = 10;
  for (int i = 0; i < GROWTH_SIZE; i++) {
    Status result = que_push(q, i);
    REQUIRE_WITH_MSG(result == OK, "Push should succeed during growth");
  }
  REQUIRE_WITH_MSG(que_len(q) == GROWTH_SIZE, "Queue should have all pushed elements");
  // Pop some elements to create gap but don't compact
  int       val       = 0;
  const int POP_COUNT = 5;
  for (int i = 0; i < POP_COUNT; i++) {
    que_pop(q, &val);
    REQUIRE_WITH_MSG(val == i, "Popped values should be in FIFO order");
  }
  _QueueHeader* h               = _que_header(q);
  size_t        capacity_before = h->capacity;
  size_t        front_before    = h->front;
  REQUIRE_WITH_MSG(front_before == POP_COUNT, "Front should advance with pops");
  REQUIRE_WITH_MSG(que_len(q) == GROWTH_SIZE - POP_COUNT,
                   "Length should decrease with pops");
  // Add more elements (should grow without compaction)
  for (int i = 100; i < 105; i++) {
    que_push(q, i);
  }
  h = _que_header(q);
  REQUIRE_WITH_MSG(h->front == front_before, "Front should not change during growth");
  REQUIRE_WITH_MSG(h->capacity >= capacity_before, "Capacity should not decrease");
  // Verify all elements are still accessible in correct order
  size_t expected_len = (GROWTH_SIZE - POP_COUNT) + 5;
  REQUIRE_WITH_MSG(que_len(q) == expected_len,
                   "Queue length should account for all operations");
  // Pop remaining original elements
  for (int i = POP_COUNT; i < GROWTH_SIZE; i++) {
    que_pop(q, &val);
    REQUIRE_WITH_MSG(val == i, "Original elements should be in correct order");
  }
  // Pop the newly added elements
  for (int i = 100; i < 105; i++) {
    que_pop(q, &val);
    REQUIRE_WITH_MSG(val == i, "New elements should be in correct order");
  }
  REQUIRE_WITH_MSG(que_is_empty(q) == true, "Queue should be empty after popping all");
  que_free(q);
}

void test_que_mixed_operations(void)
{
  int* q   = NULL;
  int  val = 0;
  // Test alternating push/pop operations
  que_push(q, 1);
  que_push(q, 2);
  que_pop(q, &val);
  REQUIRE_WITH_MSG(val == 1, "First pop should return 1");
  que_push(q, 3);
  que_push(q, 4);
  que_pop(q, &val);
  REQUIRE_WITH_MSG(val == 2, "Second pop should return 2");
  que_pop(q, &val);
  REQUIRE_WITH_MSG(val == 3, "Third pop should return 3");
  // Queue should now have front > 0 with one element (4)
  REQUIRE_WITH_MSG(que_len(q) == 1, "Queue should have 1 element");
  _QueueHeader* h = _que_header(q);
  REQUIRE_WITH_MSG(h->front > 0, "Front should be > 0 after mixed operations");
  // Test operations after mixed push/pop (ring buffer state)
  REQUIRE_WITH_MSG(que_len(q) == 1, "Queue should have 1 element");
  REQUIRE_WITH_MSG(h->front > 0, "Front should be > 0 after mixed operations");
  que_pop(q, &val);
  REQUIRE_WITH_MSG(val == 4, "Last element should be 4");
  REQUIRE_WITH_MSG(que_is_empty(q) == true, "Queue should be empty");
  // Test push after emptying
  que_push(q, 99);
  REQUIRE_WITH_MSG(que_len(q) == 1, "Queue should have 1 element after push");
  que_pop(q, &val);
  REQUIRE_WITH_MSG(val == 99, "Element should be 99");
  que_free(q);
}

void test_que_different_types(void)
{
  // Test with double
  double* dq   = NULL;
  double  dval = 0.0;
  que_push(dq, 3.14);
  que_push(dq, 2.71);
  que_pop(dq, &dval);
  REQUIRE_WITH_MSG(dval == 3.14, "Double queue FIFO behavior");
  que_free(dq);
  // Test with pointers
  const char*  strings[] = {"first", "second", "third"};
  const char** sq        = NULL;
  const char*  sval      = NULL;
  que_push(sq, strings[0]);
  que_push(sq, strings[1]);
  que_push(sq, strings[2]);
  que_pop(sq, &sval);
  REQUIRE_WITH_MSG(sval == strings[0], "String queue should return first string");
  que_pop(sq, &sval);
  REQUIRE_WITH_MSG(sval == strings[1], "String queue should return second string");
  que_free(sq);
  // Test with struct
  typedef struct
  {
    int x, y;
  } Point;
  Point* pq   = NULL;
  Point  pval = {0};
  Point  p1   = {10, 20};
  Point  p2   = {30, 40};
  que_push(pq, p1);
  que_push(pq, p2);
  que_pop(pq, &pval);
  REQUIRE_WITH_MSG(pval.x == 10 && pval.y == 20, "Struct queue should preserve data");
  que_free(pq);
}

void test_que_header_alignment(void)
{
  REQUIRE_WITH_MSG(
    sizeof(_QueueHeader) % sizeof(_MaxAlignCompat) == 0,
    "Queue header must align with the platform's maximum alignment to be compatible "
    "with arbitrary types inside the container. This doesn't guarantee alignment with "
    "SIMD types. The containers are not meant to be used with SIMD types.");
}

void test_que_ring_growth_no_wrap(void)
{
  int* q   = NULL;
  int  val = 0;
  // Test basic growth behavior through API
  que_push(q, 10);
  que_push(q, 20);
  que_push(q, 30);
  REQUIRE_WITH_MSG(que_len(q) == 3, "Length should be 3 after pushes");
  // Verify all elements are still accessible in correct order
  que_pop(q, &val);
  REQUIRE_WITH_MSG(val == 10, "First element should be 10");
  que_pop(q, &val);
  REQUIRE_WITH_MSG(val == 20, "Second element should be 20");
  que_pop(q, &val);
  REQUIRE_WITH_MSG(val == 30, "Third element should be 30");
  REQUIRE_WITH_MSG(que_is_empty(q), "Queue should be empty");
  que_free(q);
}

void test_que_ring_growth_with_wrap(void)
{
  int* q   = NULL;
  int  val = 0;
  // Fill initial capacity
  que_push(q, 10);
  que_push(q, 20);
  // Pop to advance front pointer
  que_pop(q, &val);
  REQUIRE_WITH_MSG(val == 10, "First pop should return 10");
  // Push more elements - this will test internal growth/wraparound
  que_push(q, 30);
  que_push(q, 40);
  REQUIRE_WITH_MSG(que_len(q) == 3, "Length should be 3 after mixed operations");
  // Verify elements are in correct order: [_, 20, 30, 40]
  que_pop(q, &val);
  REQUIRE_WITH_MSG(val == 20, "First element should be 20");
  que_pop(q, &val);
  REQUIRE_WITH_MSG(val == 30, "Second element should be 30");
  que_pop(q, &val);
  REQUIRE_WITH_MSG(val == 40, "Third element should be 40");
  REQUIRE_WITH_MSG(que_is_empty(q), "Queue should be empty");
  que_free(q);
}

void test_que_multiple_growths(void)
{
  int* q   = NULL;
  int  val = 0;
  // Test multiple growth cycles
  const int NUM_ELEMENTS = 20;
  // Fill queue with elements
  for (int i = 0; i < NUM_ELEMENTS; i++) {
    que_push(q, i * 10);
  }
  _QueueHeader* h = _que_header(q);
  REQUIRE_WITH_MSG(que_len(q) == NUM_ELEMENTS, "Queue should have all elements");
  REQUIRE_WITH_MSG(h->capacity >= NUM_ELEMENTS, "Capacity should be sufficient");
  // Pop half the elements to advance front
  for (int i = 0; i < NUM_ELEMENTS / 2; i++) {
    que_pop(q, &val);
    REQUIRE_WITH_MSG(val == i * 10, "Elements should pop in FIFO order");
  }
  size_t remaining = NUM_ELEMENTS - NUM_ELEMENTS / 2;
  REQUIRE_WITH_MSG(que_len(q) == remaining, "Correct number of elements should remain");
  // Add more elements to potentially trigger another growth
  for (int i = NUM_ELEMENTS; i < NUM_ELEMENTS + 10; i++) {
    que_push(q, i * 10);
  }
  REQUIRE_WITH_MSG(que_len(q) == remaining + 10, "Queue should have correct length");
  // Verify all remaining elements are in correct order
  for (int i = NUM_ELEMENTS / 2; i < NUM_ELEMENTS; i++) {
    que_pop(q, &val);
    REQUIRE_WITH_MSG(val == i * 10, "Original elements should be in order");
  }
  for (int i = NUM_ELEMENTS; i < NUM_ELEMENTS + 10; i++) {
    que_pop(q, &val);
    REQUIRE_WITH_MSG(val == i * 10, "New elements should be in order");
  }
  REQUIRE_WITH_MSG(que_is_empty(q), "Queue should be empty");
  que_free(q);
}

void test_que_stress_patterns(void)
{
  int* q   = NULL;
  int  val = 0;
  // Stress test with push/pop cycles that exercise ring buffer behavior
  int next_value = 0;
  for (int cycle = 0; cycle < 100; cycle++) {
    // Push several elements
    for (int i = 0; i < 5; i++) {
      que_push(q, next_value++);
    }
    // Pop most (but not all) to create advancing front pointer
    for (int i = 0; i < 3; i++) {
      Status result = que_pop(q, &val);
      REQUIRE_WITH_MSG(result == OK, "Pop should succeed during stress test");
    }
    // Queue should grow by 2 elements each cycle
    REQUIRE_WITH_MSG(que_len(q) == (size_t)((cycle + 1) * 2),
                     "Queue should grow predictably");
  }
  // Simply verify we can drain all remaining elements
  size_t remaining_count = que_len(q);
  for (size_t i = 0; i < remaining_count; i++) {
    Status result = que_pop(q, &val);
    REQUIRE_WITH_MSG(result == OK, "Should be able to pop all remaining elements");
  }
  REQUIRE_WITH_MSG(que_is_empty(q), "Queue should be empty after draining");
  que_free(q);
}

void test_que_edge_cases_ring(void)
{
  int* q   = NULL;
  int  val = 0;
  // Test single element in ring buffer
  que_push(q, 42);
  que_pop(q, &val);
  REQUIRE_WITH_MSG(val == 42, "Single element should work");
  // Push again after empty
  que_push(q, 99);
  REQUIRE_WITH_MSG(que_len(q) == 1, "Length should be 1");
  // Force growth from single element state
  que_push(q, 100);
  que_push(q, 101);  // This should trigger growth
  REQUIRE_WITH_MSG(que_len(q) == 3, "Should have 3 elements");
  // Verify order
  que_pop(q, &val);
  REQUIRE_WITH_MSG(val == 99, "First element should be 99");
  que_pop(q, &val);
  REQUIRE_WITH_MSG(val == 100, "Second element should be 100");
  que_pop(q, &val);
  REQUIRE_WITH_MSG(val == 101, "Third element should be 101");
  REQUIRE_WITH_MSG(que_is_empty(q), "Queue should be empty");
  que_free(q);
}

void test_que_capacity_doubling(void)
{
  int* q = NULL;
  // Test that capacity doubles correctly
  // Growth happens when buffer would become full
  size_t expected_capacity = 2;
  for (int i = 0; i < 16; i++) {
    que_push(q, i);
    _QueueHeader* h = _que_header(q);
    // Growth happens when the next push would make (1+back)%capacity == front
    // For a ring buffer starting at front=0, this happens when back+1 == capacity
    // So capacity doubles when we have capacity-1 elements
    if (h->back == expected_capacity) {
      expected_capacity *= 2;
    }
    REQUIRE_WITH_MSG(h->capacity == expected_capacity,
                     "Capacity should match expected doubling pattern");
  }
  REQUIRE_WITH_MSG(que_len(q) == 16, "Should have all 16 elements");
  // Verify all elements are correct
  int val = 0;
  for (int i = 0; i < 16; i++) {
    que_pop(q, &val);
    REQUIRE_WITH_MSG(val == i, "Elements should be in FIFO order");
  }
  que_free(q);
}

typedef struct
{
  size_t nslots;
  size_t nbuckets;
  size_t n_used;
  size_t n_removed;
} HMapStats;

static HMapStats _hmap_stats(void* ptr)
{
  _HashTableHeader* h = _hmap_header(ptr);
  if (h) {
    size_t const nbuckets = h->n_total / HMAP_BUCKET_SIZE;
    REQUIRE(nbuckets == _arr_capacity(h->buckets));
    REQUIRE(nbuckets == arr_len(h->buckets));
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
  Entry* map = NULL;
  // Test empty map
  REQUIRE_WITH_MSG(hmap_len(map) == 0, "Empty map should have length 0");
  HMapStats stats = _hmap_stats(map);
  REQUIRE_WITH_MSG(stats.n_used == 0, "Empty map should have 0 used slots");
  REQUIRE_WITH_MSG(stats.nslots == 0, "Empty map should have 0 total slots");
  REQUIRE_WITH_MSG(stats.nbuckets == 0, "Empty map should have 0 buckets");
  // Test single insertion
  int key1 = 42, val1 = 100;
  hmap_put(map, key1, val1);
  REQUIRE_WITH_MSG(hmap_len(map) == 1, "Map should have 1 element after insert");
  REQUIRE_WITH_MSG(map[0].key == 42, "First entry key should be 42");
  REQUIRE_WITH_MSG(map[0].value == 100, "First entry value should be 100");
  stats = _hmap_stats(map);
  REQUIRE_WITH_MSG(stats.n_used == 1, "Should have 1 used slot after insert");
  REQUIRE_WITH_MSG(stats.nslots == HMAP_BUCKET_SIZE,
                   "Initial size should be HMAP_BUCKET_SIZE slots");
  REQUIRE_WITH_MSG(stats.nbuckets == 1, "Should have 1 bucket initially");
  REQUIRE_WITH_MSG(stats.n_removed == 0, "Should have no removed entries");
  // Test update (same key)
  int val2 = 200;
  hmap_put(map, key1, val2);
  REQUIRE_WITH_MSG(hmap_len(map) == 1, "Map should still have 1 element after update");
  REQUIRE_WITH_MSG(map[0].key == 42, "Key should remain 42");
  REQUIRE_WITH_MSG(map[0].value == 200, "Value should be updated to 200");
  stats = _hmap_stats(map);
  REQUIRE_WITH_MSG(stats.n_used == 1, "Should still have 1 used slot after update");
  REQUIRE_WITH_MSG(stats.nslots == HMAP_BUCKET_SIZE,
                   "Size should remain unchanged after update");
  REQUIRE_WITH_MSG(stats.n_removed == 0, "Update shouldn't create removed entries");
  // Test second insertion (different key)
  int key2 = 99, val3 = 300;
  hmap_put(map, key2, val3);
  REQUIRE_WITH_MSG(hmap_len(map) == 2, "Map should have 2 elements");
  stats = _hmap_stats(map);
  REQUIRE_WITH_MSG(stats.n_used == 2, "Should have 2 used slots after second insert");
  REQUIRE_WITH_MSG(stats.nslots == HMAP_BUCKET_SIZE, "Size should still be initial size");
  REQUIRE_WITH_MSG(stats.n_removed == 0, "Should still have no removed entries");
  // Verify both entries exist (order may vary due to hashing)
  bool found_42 = false, found_99 = false;
  for (size_t i = 0; i < hmap_len(map); i++) {
    if (map[i].key == 42) {
      REQUIRE_WITH_MSG(map[i].value == 200, "Key 42 should have value 200");
      found_42 = true;
    }
    else if (map[i].key == 99) {
      REQUIRE_WITH_MSG(map[i].value == 300, "Key 99 should have value 300");
      found_99 = true;
    }
  }
  REQUIRE_WITH_MSG(found_42, "Should find key 42 in map");
  REQUIRE_WITH_MSG(found_99, "Should find key 99 in map");
  hmap_free(map);
}

void test_hmap_growth(void)
{
  typedef struct
  {
    int key;
    int value;
  } Entry;
  Entry* map = NULL;
  // Insert enough elements to trigger growth (initial size is 8, growth at 75%)
  // So we need more than 6 elements to trigger growth
  for (int i = 0; i < 10; i++) {
    int key = i, value = i * 10;
    hmap_put(map, key, value);
  }
  REQUIRE_WITH_MSG(hmap_len(map) == 10, "All elements should be inserted");
  // Verify all elements are present
  bool found[10] = {false};
  for (size_t i = 0; i < hmap_len(map); i++) {
    int key            = map[i].key;
    int expected_value = key * 10;
    REQUIRE_WITH_MSG(key >= 0 && key < 10, "Key should be in valid range");
    REQUIRE_WITH_MSG(map[i].value == expected_value, "Value should match expected");
    found[key] = true;
  }
  for (int i = 0; i < 10; i++) {
    REQUIRE_WITH_MSG(found[i], "All keys should be found after growth");
  }
  hmap_free(map);
}

void test_hmap_edge_cases(void)
{
  typedef struct
  {
    int key;
    int value;
  } Entry;
  Entry* map = NULL;
  // Test with key 0 (potential edge case)
  int key0 = 0, val0 = 999;
  hmap_put(map, key0, val0);
  REQUIRE_WITH_MSG(hmap_len(map) == 1, "Should handle key 0");
  REQUIRE_WITH_MSG(map[0].key == 0, "Key 0 should be stored correctly");
  REQUIRE_WITH_MSG(map[0].value == 999, "Value for key 0 should be correct");
  // Test with negative keys
  int key_neg = -1, val_neg = -999;
  hmap_put(map, key_neg, val_neg);
  REQUIRE_WITH_MSG(hmap_len(map) == 2, "Should handle negative keys");
  // Test with large keys
  int key_large = 1000000, val_large = 123;
  hmap_put(map, key_large, val_large);
  REQUIRE_WITH_MSG(hmap_len(map) == 3, "Should handle large keys");
  // Test multiple updates to same key
  int key_repeat = 42;
  for (int i = 0; i < 5; i++) {
    hmap_put(map, key_repeat, i);
    REQUIRE_WITH_MSG(hmap_len(map) == 4, "Multiple updates shouldn't increase size");
  }
  // Find key 42 and verify final value
  bool found_42 = false;
  for (size_t i = 0; i < hmap_len(map); i++) {
    if (map[i].key == 42) {
      REQUIRE_WITH_MSG(map[i].value == 4, "Final update value should be 4");
      found_42 = true;
      break;
    }
  }
  REQUIRE_WITH_MSG(found_42, "Should find key 42 after multiple updates");
  hmap_free(map);
}

void test_hmap_null_operations(void)
{
  typedef struct
  {
    int key;
    int value;
  } Entry;
  Entry* null_map = NULL;
  // Test length of NULL map
  REQUIRE_WITH_MSG(hmap_len(null_map) == 0, "NULL map should have length 0");
  // Test that we can insert into NULL map (should initialize)
  int key = 1, value = 100;
  hmap_put(null_map, key, value);
  REQUIRE_WITH_MSG(hmap_len(null_map) == 1, "Should initialize NULL map on first insert");
  REQUIRE_WITH_MSG(null_map[0].key == 1, "First key should be correct");
  REQUIRE_WITH_MSG(null_map[0].value == 100, "First value should be correct");
  hmap_free(null_map);
}

void test_hmap_stress_test(void)
{
  typedef struct
  {
    int key;
    int value;
  } Entry;
  Entry* map = NULL;
  // Test many insertions to stress growth/rehashing
  // Insert many elements
  for (int i = 0; i < 1000; i++) {
    int key = i, value = i * 2;
    hmap_put(map, key, value);
  }
  REQUIRE_WITH_MSG(hmap_len(map) == 1000, "Should handle many insertions");
  HMapStats stats = _hmap_stats(map);
  REQUIRE_WITH_MSG(stats.n_used == 1000, "Stats should match actual count");
  REQUIRE_WITH_MSG(stats.n_removed == 0,
                   "Should have no removed entries after insertions");
  // Size should be at least large enough, and a multiple of HMAP_BUCKET_SIZE
  REQUIRE_WITH_MSG(stats.nslots >= 1000, "Should have grown to accommodate all elements");
  REQUIRE_WITH_MSG(stats.nslots % HMAP_BUCKET_SIZE == 0,
                   "Slot count should be multiple of bucket size");
  REQUIRE_WITH_MSG(stats.nbuckets == stats.nslots / HMAP_BUCKET_SIZE,
                   "Bucket count should be consistent");
  // Verify all elements are still there after multiple growths
  bool found[1000] = {false};
  for (size_t i = 0; i < hmap_len(map); i++) {
    int key            = map[i].key;
    int expected_value = key * 2;
    REQUIRE_WITH_MSG(key >= 0 && key < 1000, "Key should be in valid range");
    REQUIRE_WITH_MSG(map[i].value == expected_value,
                     "Value should be correct after growth");
    found[key] = true;
  }
  // Check that all keys were found
  for (int i = 0; i < 1000; i++) {
    REQUIRE_WITH_MSG(found[i], "All keys should survive multiple growths");
  }
  // Test many updates
  for (int i = 0; i < 1000; i++) {
    int key = i, new_value = i * 3;
    hmap_put(map, key, new_value);
  }
  REQUIRE_WITH_MSG(hmap_len(map) == 1000, "Length should remain same after updates");
  stats = _hmap_stats(map);
  REQUIRE_WITH_MSG(stats.n_used == 1000, "Used count should remain same after updates");
  REQUIRE_WITH_MSG(stats.n_removed == 0, "Updates shouldn't create removed entries");
  // Verify updates worked
  for (size_t i = 0; i < hmap_len(map); i++) {
    int key            = map[i].key;
    int expected_value = key * 3;
    REQUIRE_WITH_MSG(map[i].value == expected_value, "Updated values should be correct");
  }
  hmap_free(map);
}

void test_hmap_hash_collision_simulation(void)
{
  typedef struct
  {
    int key;
    int value;
  } Entry;
  Entry* map = NULL;
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
    hmap_put(map, key, value);
  }
  REQUIRE_WITH_MSG(hmap_len(map) == (size_t)num_keys,
                   "Should handle potential hash collisions");
  // Verify all keys are present and correct
  for (int i = 0; i < num_keys; i++) {
    int  target_key     = collision_keys[i];
    int  expected_value = i + 100;
    bool found          = false;
    for (size_t j = 0; j < hmap_len(map); j++) {
      if (map[j].key == target_key) {
        REQUIRE_WITH_MSG(map[j].value == expected_value,
                         "Collision key should have correct value");
        found = true;
        break;
      }
    }
    REQUIRE_WITH_MSG(found, "All collision-prone keys should be found");
  }
  hmap_free(map);
}

void test_hmap_boundary_conditions(void)
{
  typedef struct
  {
    int key;
    int value;
  } Entry;
  Entry* map = NULL;
  // Test exactly at growth boundaries
  // Initial size is HMAP_BUCKET_SIZE, grows at 75% = 6 elements (assuming
  // HMAP_BUCKET_SIZE=8)
  // Insert exactly to growth threshold
  for (int i = 0; i < 6; i++) {
    int key = i, value = i;
    hmap_put(map, key, value);
  }
  REQUIRE_WITH_MSG(hmap_len(map) == 6, "Should handle exactly 6 elements");
  HMapStats stats = _hmap_stats(map);
  REQUIRE_WITH_MSG(stats.n_used == 6, "Should have 6 used slots at threshold");
  REQUIRE_WITH_MSG(stats.nslots == HMAP_BUCKET_SIZE,
                   "Should still be initial size before growth");
  REQUIRE_WITH_MSG(stats.nbuckets == 1, "Should still have 1 bucket before growth");
  REQUIRE_WITH_MSG(stats.n_removed == 0, "Should have no removed entries");
  // One more should trigger growth
  int key7 = 100, val7 = 200;
  hmap_put(map, key7, val7);
  REQUIRE_WITH_MSG(hmap_len(map) == 7, "Should handle growth trigger");
  stats = _hmap_stats(map);
  REQUIRE_WITH_MSG(stats.n_used == 7, "Should have 7 used slots after growth");
  REQUIRE_WITH_MSG(stats.nslots == HMAP_BUCKET_SIZE * 2,
                   "Should double in size after growth");
  REQUIRE_WITH_MSG(stats.nbuckets == 2, "Should have 2 buckets after growth");
  REQUIRE_WITH_MSG(stats.n_removed == 0, "Growth should reset removed count to 0");
  // Verify all elements survive growth
  bool found[7]  = {false};
  bool found_100 = false;
  for (size_t i = 0; i < hmap_len(map); i++) {
    if (map[i].key == 100) {
      REQUIRE_WITH_MSG(map[i].value == 200, "Growth trigger element should be correct");
      found_100 = true;
    }
    else if (map[i].key >= 0 && map[i].key < 6) {
      found[map[i].key] = true;
      REQUIRE_WITH_MSG(map[i].value == map[i].key,
                       "Original elements should survive growth");
    }
  }
  REQUIRE_WITH_MSG(found_100, "Growth trigger element should be found");
  for (int i = 0; i < 6; i++) {
    REQUIRE_WITH_MSG(found[i], "All original elements should survive growth");
  }
  hmap_free(map);
}

void test_hmap_extreme_values(void)
{
  typedef struct
  {
    int       key;
    long long value;  // Use larger value type
  } Entry;
  Entry* map = NULL;
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
    hmap_put(map, key, value);
  }
  REQUIRE_WITH_MSG(hmap_len(map) == (size_t)num_keys, "Should handle extreme key values");
  // Verify extreme values
  for (int i = 0; i < num_keys; i++) {
    int       target_key     = extreme_keys[i];
    long long expected_value = (long long)target_key * 1000000LL;
    bool      found          = false;
    for (size_t j = 0; j < hmap_len(map); j++) {
      if (map[j].key == target_key) {
        REQUIRE_WITH_MSG(map[j].value == expected_value,
                         "Extreme value should be correct");
        found = true;
        break;
      }
    }
    REQUIRE_WITH_MSG(found, "Extreme key should be found");
  }
  hmap_free(map);
}

void test_hmap_repeated_growth(void)
{
  typedef struct
  {
    int key;
    int value;
  } Entry;
  Entry* map = NULL;
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
      hmap_put(map, key, value);
    }
    total_inserted += phase_size;
    REQUIRE_WITH_MSG(hmap_len(map) == (size_t)total_inserted,
                     "Length should match total insertions");
    // Verify all previous elements are still correct after each growth
    for (int check_key = 0; check_key < total_inserted; check_key++) {
      bool found = false;
      for (size_t j = 0; j < hmap_len(map); j++) {
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
          REQUIRE_WITH_MSG(map[j].value == expected_value,
                           "Value should survive multiple growths");
          found = true;
          break;
        }
      }
      REQUIRE_WITH_MSG(found, "All keys should survive repeated growths");
    }
  }
  hmap_free(map);
}

void test_hmap_get_basic(void)
{
  typedef struct
  {
    int key;
    int value;
  } Entry;
  Entry* map = NULL;
  // Test get from empty map
  int    key    = 42;
  Entry* result = (Entry*)hmap_get(map, key);
  REQUIRE_WITH_MSG(result == NULL, "Get from empty map should return NULL");
  REQUIRE_WITH_MSG(!hmap_contains(map, key), "Empty map should not contain any key");
  // Insert some entries
  int key1 = 10, val1 = 100;
  int key2 = 20, val2 = 200;
  int key3 = 30, val3 = 300;
  hmap_put(map, key1, val1);
  hmap_put(map, key2, val2);
  hmap_put(map, key3, val3);
  // Test hmap_contains
  REQUIRE_WITH_MSG(hmap_contains(map, key1), "Should contain key1");
  REQUIRE_WITH_MSG(hmap_contains(map, key2), "Should contain key2");
  REQUIRE_WITH_MSG(hmap_contains(map, key3), "Should contain key3");
  int missing_key1 = 999;
  REQUIRE_WITH_MSG(!hmap_contains(map, missing_key1), "Should not contain missing key");
  // Test successful gets
  result = (Entry*)hmap_get(map, key1);
  REQUIRE_WITH_MSG(result != NULL, "Should find existing key 10");
  REQUIRE_WITH_MSG(result->key == 10, "Retrieved entry should have correct key");
  REQUIRE_WITH_MSG(result->value == 100, "Retrieved entry should have correct value");
  result = (Entry*)hmap_get(map, key2);
  REQUIRE_WITH_MSG(result != NULL, "Should find existing key 20");
  REQUIRE_WITH_MSG(result->key == 20, "Retrieved entry should have correct key");
  REQUIRE_WITH_MSG(result->value == 200, "Retrieved entry should have correct value");
  result = (Entry*)hmap_get(map, key3);
  REQUIRE_WITH_MSG(result != NULL, "Should find existing key 30");
  REQUIRE_WITH_MSG(result->key == 30, "Retrieved entry should have correct key");
  REQUIRE_WITH_MSG(result->value == 300, "Retrieved entry should have correct value");
  // Test missing key
  int missing_key = 999;
  result          = (Entry*)hmap_get(map, missing_key);
  REQUIRE_WITH_MSG(result == NULL, "Should return NULL for missing key");
  REQUIRE_WITH_MSG(!hmap_contains(map, missing_key), "Should not contain missing key");
  hmap_free(map);
}

void test_hmap_get_after_updates(void)
{
  typedef struct
  {
    int key;
    int value;
  } Entry;
  Entry* map = NULL;
  // Insert initial value
  int key = 42, initial_val = 100;
  hmap_put(map, key, initial_val);
  // Verify initial state
  REQUIRE_WITH_MSG(hmap_contains(map, key), "Should contain key after insert");
  Entry* result = (Entry*)hmap_get(map, key);
  REQUIRE_WITH_MSG(result != NULL, "Should find key after initial insert");
  REQUIRE_WITH_MSG(result->value == 100, "Should have initial value");
  // Update the value
  int updated_val = 999;
  hmap_put(map, key, updated_val);
  // Verify state after update
  REQUIRE_WITH_MSG(hmap_contains(map, key), "Should still contain key after update");
  result = (Entry*)hmap_get(map, key);
  REQUIRE_WITH_MSG(result != NULL, "Should still find key after update");
  REQUIRE_WITH_MSG(result->value == 999, "Should have updated value");
  REQUIRE_WITH_MSG(result->key == 42, "Key should remain unchanged");
  // Verify map still has only one entry
  REQUIRE_WITH_MSG(hmap_len(map) == 1,
                   "Map should still have only one entry after update");
  hmap_free(map);
}

void test_hmap_get_with_collisions(void)
{
  typedef struct
  {
    int key;
    int value;
  } Entry;
  Entry* map = NULL;
  // Use keys that are likely to cause collisions
  int collision_keys[] = {0, 8, 16, 24, 32};  // Multiples of 8
  int values[]         = {100, 200, 300, 400, 500};
  int num_keys         = sizeof(collision_keys) / sizeof(collision_keys[0]);
  // Insert collision-prone keys
  for (int i = 0; i < num_keys; i++) {
    hmap_put(map, collision_keys[i], values[i]);
  }
  // Verify all keys are contained
  for (int i = 0; i < num_keys; i++) {
    REQUIRE_WITH_MSG(hmap_contains(map, collision_keys[i]),
                     "Should contain collision-prone key");
  }
  // Verify all keys can be retrieved correctly
  for (int i = 0; i < num_keys; i++) {
    Entry* result = (Entry*)hmap_get(map, collision_keys[i]);
    REQUIRE_WITH_MSG(result != NULL, "Should find collision-prone key");
    REQUIRE_WITH_MSG(result->key == collision_keys[i], "Retrieved key should match");
    REQUIRE_WITH_MSG(result->value == values[i], "Retrieved value should match");
  }
  // Test missing keys that might hash to same buckets
  int missing_keys[] = {40, 48, 56};
  for (int i = 0; i < 3; i++) {
    REQUIRE_WITH_MSG(!hmap_contains(map, missing_keys[i]),
                     "Should not contain missing collision-candidate key");
    Entry* result = (Entry*)hmap_get(map, missing_keys[i]);
    REQUIRE_WITH_MSG(result == NULL, "Should not find missing collision-candidate key");
  }
  hmap_free(map);
}

void test_hmap_get_after_growth(void)
{
  typedef struct
  {
    int key;
    int value;
  } Entry;
  Entry* map = NULL;
  // Insert elements before growth
  int pre_growth_keys[]   = {1, 2, 3, 4, 5};
  int pre_growth_values[] = {10, 20, 30, 40, 50};
  for (int i = 0; i < 5; i++) {
    hmap_put(map, pre_growth_keys[i], pre_growth_values[i]);
  }
  // Verify pre-growth containment and retrieval
  for (int i = 0; i < 5; i++) {
    REQUIRE_WITH_MSG(hmap_contains(map, pre_growth_keys[i]),
                     "Should contain pre-growth key");
    Entry* result = (Entry*)hmap_get(map, pre_growth_keys[i]);
    REQUIRE_WITH_MSG(result != NULL, "Should find pre-growth key");
    REQUIRE_WITH_MSG(result->value == pre_growth_values[i],
                     "Pre-growth value should be correct");
  }
  // Trigger growth by adding more elements (assuming 8 initial size, 75% threshold)
  int post_growth_keys[]   = {6, 7, 8, 9, 10};
  int post_growth_values[] = {60, 70, 80, 90, 100};
  for (int i = 0; i < 5; i++) {
    hmap_put(map, post_growth_keys[i], post_growth_values[i]);
  }
  // Verify all pre-growth entries still accessible after growth
  for (int i = 0; i < 5; i++) {
    REQUIRE_WITH_MSG(hmap_contains(map, pre_growth_keys[i]),
                     "Should contain pre-growth key after growth");
    Entry* result = (Entry*)hmap_get(map, pre_growth_keys[i]);
    REQUIRE_WITH_MSG(result != NULL, "Should find pre-growth key after growth");
    REQUIRE_WITH_MSG(result->value == pre_growth_values[i],
                     "Pre-growth value should survive growth");
  }
  // Verify post-growth entries
  for (int i = 0; i < 5; i++) {
    REQUIRE_WITH_MSG(hmap_contains(map, post_growth_keys[i]),
                     "Should contain post-growth key");
    Entry* result = (Entry*)hmap_get(map, post_growth_keys[i]);
    REQUIRE_WITH_MSG(result != NULL, "Should find post-growth key");
    REQUIRE_WITH_MSG(result->value == post_growth_values[i],
                     "Post-growth value should be correct");
  }
  hmap_free(map);
}

void test_hmap_get_edge_cases(void)
{
  typedef struct
  {
    int       key;
    long long value;  // Different value type
  } Entry;
  Entry* map = NULL;
  // Test with extreme key values
  int       extreme_keys[]   = {INT_MAX, INT_MIN, 0, -1, 1};
  long long extreme_values[] = {1000000LL, -1000000LL, 0LL, -1LL, 1LL};
  for (int i = 0; i < 5; i++) {
    hmap_put(map, extreme_keys[i], extreme_values[i]);
  }
  // Verify extreme values with both contains and get
  for (int i = 0; i < 5; i++) {
    REQUIRE_WITH_MSG(hmap_contains(map, extreme_keys[i]), "Should contain extreme key");
    Entry* result = (Entry*)hmap_get(map, extreme_keys[i]);
    REQUIRE_WITH_MSG(result != NULL, "Should find extreme key");
    REQUIRE_WITH_MSG(result->key == extreme_keys[i], "Extreme key should match");
    REQUIRE_WITH_MSG(result->value == extreme_values[i], "Extreme value should match");
  }
  // Test key 0 specifically (potential edge case)
  int zero_key = 0;
  REQUIRE_WITH_MSG(hmap_contains(map, zero_key), "Should contain key 0");
  Entry* result = (Entry*)hmap_get(map, zero_key);
  REQUIRE_WITH_MSG(result != NULL, "Should find key 0");
  REQUIRE_WITH_MSG(result->key == 0, "Key 0 should be retrievable");
  REQUIRE_WITH_MSG(result->value == 0LL, "Value for key 0 should be correct");
  hmap_free(map);
}

void test_hmap_get_null_safety(void)
{
  typedef struct
  {
    int key;
    int value;
  } Entry;
  // Test operations on NULL map
  Entry* null_map      = NULL;
  int    test_key_null = 42;
  REQUIRE_WITH_MSG(!hmap_contains(null_map, test_key_null),
                   "NULL map should not contain any key");
  Entry* result = (Entry*)hmap_get(null_map, test_key_null);
  REQUIRE_WITH_MSG(result == NULL, "Get from NULL map should return NULL");
  // Test get from map that works normally
  Entry* map = NULL;
  int    key = 123, value = 456;
  hmap_put(map, key, value);
  // Verify it works before free
  REQUIRE_WITH_MSG(hmap_contains(map, key), "Should contain key before free");
  result = (Entry*)hmap_get(map, key);
  REQUIRE_WITH_MSG(result != NULL, "Should find key before free");
  hmap_free(map);
  // Note: Don't test after free as that would be undefined behavior
}

void print_hmap(void* ptr)  // DEBUG.
{
  typedef struct
  {
    uint64_t key;
    uint64_t value;
  } Entry;
  Entry* map = (Entry*)ptr;
  if (map == NULL) {
    printf("\nMap is NULL\n");
    return;
  }
  _HashTableHeader* h = _hmap_header(map);
  printf("\nPairs (%zu):\n", h->n_total);
  size_t const npairs = h->n_total;
  for (size_t i = 0; i < npairs; ++i, ++map) {
    printf("\t[%zu]: (k: %" PRIu64 "; v: %" PRIu64 ")\n", i, map->key, map->value);
  }
  printf("Buckets:\n");
  _HashBucket* bk  = h->buckets;
  size_t const nbs = h->n_total / HMAP_BUCKET_SIZE;
  for (size_t i = 0; i < nbs; ++i, ++bk) {
    printf("\t-----------------------\n");
    for (size_t s = 0; s < HMAP_BUCKET_SIZE; ++s) {
      printf("\t\t(h: %zu; i: %zi)\n", bk->hash[s], bk->index[s]);
    }
  }
}

void test_hmap_fibo_indices(void)
{
  typedef struct
  {
    uint64_t key;
    uint64_t value;
  } Entry;
  uint64_t* fibo = NULL;
  {  // Populate array.
    REQUIRE_WITH_MSG(arr_push(fibo, 1) == OK, "Failed to push to array");
    REQUIRE_WITH_MSG(arr_push(fibo, 1) == OK, "Failed to push to array");
    for (size_t i = 0; i < 50; ++i) {
      size_t const len = arr_len(fibo);
      REQUIRE_WITH_MSG(arr_push(fibo, fibo[len - 2] + fibo[len - 1]) == OK,
                       "Array length is not correct.");
    }
    REQUIRE_WITH_MSG(arr_len(fibo) == 52, "Not enough fibonacci numbers.");
    arr_remove(fibo, 0);  // Remove the duplicated 1.
    REQUIRE_WITH_MSG(arr_len(fibo) == 51, "One less after removing the duplicate");
  }
  Entry* idxmap = NULL;
  {  // Populate the map.
    size_t const len = arr_len(fibo);
    uint64_t*    fn  = fibo;
    for (size_t i = 0; i < len; ++i, ++fn) {
      hmap_put(idxmap, *fn, i);
      REQUIRE_WITH_MSG(hmap_len(idxmap) == (i + 1), "Hasmap size is not growing.");
    }
  }
  REQUIRE_WITH_MSG(hmap_len(idxmap) == arr_len(fibo),
                   "Hashmap o fibonacci numbers is not the right size.");
  {  // Check the mapping.
    size_t const len = arr_len(fibo);
    uint64_t*    fn  = fibo;
    for (size_t i = 0; i < len; ++i, ++fn) {
      Entry* match = (Entry*)hmap_get(idxmap, *fn);
      REQUIRE_WITH_MSG(match != NULL, "Match must be found.");
      REQUIRE_WITH_MSG(match->key == *fn && match->value == i,
                       "Match doesn't actually match");
    }
  }
  hmap_free(idxmap);
  arr_free(fibo);
}

void test_hmap_remove_basic(void)
{
  typedef struct
  {
    int key;
    int value;
  } Entry;
  Entry* map = NULL;
  // Insert some elements
  int key1 = 10, val1 = 100;
  int key2 = 20, val2 = 200;
  int key3 = 30, val3 = 300;
  hmap_put(map, key1, val1);
  hmap_put(map, key2, val2);
  hmap_put(map, key3, val3);
  REQUIRE_WITH_MSG(hmap_len(map) == 3, "Should have 3 elements before removal");
  HMapStats stats = _hmap_stats(map);
  REQUIRE_WITH_MSG(stats.n_used == 3, "Should have 3 used slots");
  REQUIRE_WITH_MSG(stats.n_removed == 0, "Should have 0 removed slots initially");
  // Test successful removal
  hmap_remove(map, key2);
  REQUIRE_WITH_MSG(hmap_len(map) == 2, "Should have 2 elements after removal");
  REQUIRE_WITH_MSG(!hmap_contains(map, key2), "Should not contain removed key");
  Entry* result = (Entry*)hmap_get(map, key2);
  REQUIRE_WITH_MSG(result == NULL, "Get should return NULL for removed key");
  stats = _hmap_stats(map);
  REQUIRE_WITH_MSG(stats.n_used == 2, "Should have 2 used slots after removal");
  REQUIRE_WITH_MSG(stats.n_removed <= 1,
                   "Should have at most 1 removed slot after removal");
  // Verify other elements still exist
  REQUIRE_WITH_MSG(hmap_contains(map, key1), "Should still contain key1");
  REQUIRE_WITH_MSG(hmap_contains(map, key3), "Should still contain key3");
  result = (Entry*)hmap_get(map, key1);
  REQUIRE_WITH_MSG(result != NULL, "Should find key1");
  REQUIRE_WITH_MSG(result->value == val1, "Key1 should have correct value");
  result = (Entry*)hmap_get(map, key3);
  REQUIRE_WITH_MSG(result != NULL, "Should find key3");
  REQUIRE_WITH_MSG(result->value == val3, "Key3 should have correct value");
  // Remove remaining elements
  hmap_remove(map, key1);
  hmap_remove(map, key3);
  REQUIRE_WITH_MSG(hmap_len(map) == 0, "Should be empty after removing all");
  stats = _hmap_stats(map);
  REQUIRE_WITH_MSG(stats.n_used == 0, "Should have 0 used slots when empty");
  REQUIRE_WITH_MSG(stats.n_removed <= 3,
                   "Should have at most 3 removed slots (may compact)");
  hmap_free(map);
}

void test_hmap_remove_nonexistent(void)
{
  typedef struct
  {
    int key;
    int value;
  } Entry;
  Entry* map = NULL;
  // Test removing from empty map
  int missing_key = 999;
  hmap_remove(map, missing_key);  // Should do nothing
  REQUIRE_WITH_MSG(hmap_len(map) == 0, "Empty map should remain empty");
  // Insert some elements
  int key1 = 10, val1 = 100;
  int key2 = 20, val2 = 200;
  hmap_put(map, key1, val1);
  hmap_put(map, key2, val2);
  HMapStats stats           = _hmap_stats(map);
  size_t    initial_used    = stats.n_used;
  size_t    initial_removed = stats.n_removed;
  size_t    initial_len     = hmap_len(map);
  // Try to remove non-existent keys
  int nonexistent_keys[] = {5, 15, 25, 999, -1, 0};
  for (int i = 0; i < 6; i++) {
    hmap_remove(map, nonexistent_keys[i]);
    // Verify map state unchanged
    REQUIRE_WITH_MSG(hmap_len(map) == initial_len, "Length should not change");
    stats = _hmap_stats(map);
    REQUIRE_WITH_MSG(stats.n_used == initial_used, "Used count should not change");
    REQUIRE_WITH_MSG(stats.n_removed == initial_removed,
                     "Removed count should not change");
    // Verify existing keys still present
    REQUIRE_WITH_MSG(hmap_contains(map, key1), "Existing key1 should still be present");
    REQUIRE_WITH_MSG(hmap_contains(map, key2), "Existing key2 should still be present");
  }
  // Remove an existing key, then try to remove it again
  hmap_remove(map, key1);
  REQUIRE_WITH_MSG(!hmap_contains(map, key1), "Key1 should be removed");
  stats                        = _hmap_stats(map);
  size_t used_after_removal    = stats.n_used;
  size_t removed_after_removal = stats.n_removed;
  // Try to remove the already removed key
  hmap_remove(map, key1);
  stats = _hmap_stats(map);
  REQUIRE_WITH_MSG(stats.n_used == used_after_removal,
                   "Used count should not change when removing already removed key");
  REQUIRE_WITH_MSG(stats.n_removed == removed_after_removal,
                   "Removed count should not change when removing already removed key");
  REQUIRE_WITH_MSG(!hmap_contains(map, key1), "Key1 should still not be present");
  REQUIRE_WITH_MSG(hmap_contains(map, key2), "Key2 should still be present");
  hmap_free(map);
}

void test_hmap_remove_with_collisions(void)
{
  typedef struct
  {
    int key;
    int value;
  } Entry;
  Entry* map = NULL;
  // Use keys that are likely to cause collisions
  int collision_keys[] = {0, 8, 16, 24, 32, 40};  // Multiples of 8
  int values[]         = {100, 200, 300, 400, 500, 600};
  int num_keys         = sizeof(collision_keys) / sizeof(collision_keys[0]);
  // Insert collision-prone keys
  for (int i = 0; i < num_keys; i++) {
    hmap_put(map, collision_keys[i], values[i]);
  }
  REQUIRE_WITH_MSG(hmap_len(map) == (size_t)num_keys, "Should have all keys initially");
  // Remove middle element (should be in a collision chain)
  int removed_key   = collision_keys[2];  // key = 16
  int removed_value = values[2];          // value = 300
  REQUIRE_WITH_MSG(hmap_contains(map, removed_key), "Key should exist before removal");
  Entry* result = (Entry*)hmap_get(map, removed_key);
  REQUIRE_WITH_MSG(result != NULL && result->value == removed_value,
                   "Should find correct value before removal");
  hmap_remove(map, removed_key);
  REQUIRE_WITH_MSG(hmap_len(map) == (size_t)num_keys - 1, "Length should decrease by 1");
  REQUIRE_WITH_MSG(!hmap_contains(map, removed_key), "Removed key should not be found");
  result = (Entry*)hmap_get(map, removed_key);
  REQUIRE_WITH_MSG(result == NULL, "Get should return NULL for removed key");
  // Verify all other collision-prone keys are still accessible
  for (int i = 0; i < num_keys; i++) {
    if (collision_keys[i] == removed_key)
      continue;
    REQUIRE_WITH_MSG(hmap_contains(map, collision_keys[i]),
                     "Other collision keys should still be present");
    result = (Entry*)hmap_get(map, collision_keys[i]);
    REQUIRE_WITH_MSG(result != NULL, "Should find other collision keys");
    REQUIRE_WITH_MSG(result->key == collision_keys[i], "Key should match");
    REQUIRE_WITH_MSG(result->value == values[i], "Value should match");
  }
  // Remove first element in potential chain
  hmap_remove(map, collision_keys[0]);
  REQUIRE_WITH_MSG(!hmap_contains(map, collision_keys[0]), "First key should be removed");
  // Remove last element in potential chain
  hmap_remove(map, collision_keys[num_keys - 1]);
  REQUIRE_WITH_MSG(!hmap_contains(map, collision_keys[num_keys - 1]),
                   "Last key should be removed");
  // Verify remaining elements are still accessible
  for (int i = 1; i < num_keys - 1; i++) {
    if (collision_keys[i] == removed_key)
      continue;
    REQUIRE_WITH_MSG(hmap_contains(map, collision_keys[i]),
                     "Remaining collision keys should still be accessible");
  }
  hmap_free(map);
}

void test_hmap_remove_after_growth(void)
{
  typedef struct
  {
    int key;
    int value;
  } Entry;
  Entry* map = NULL;
  // Insert elements before growth (assuming HMAP_BUCKET_SIZE=8, threshold=75%)
  int pre_growth_keys[]   = {1, 2, 3, 4, 5, 6};
  int pre_growth_values[] = {10, 20, 30, 40, 50, 60};
  for (int i = 0; i < 6; i++) {
    hmap_put(map, pre_growth_keys[i], pre_growth_values[i]);
  }
  HMapStats stats         = _hmap_stats(map);
  size_t    initial_slots = stats.nslots;
  // Trigger growth
  int growth_keys[]   = {7, 8, 9, 10};
  int growth_values[] = {70, 80, 90, 100};
  for (int i = 0; i < 4; i++) {
    hmap_put(map, growth_keys[i], growth_values[i]);
  }
  stats = _hmap_stats(map);
  REQUIRE_WITH_MSG(stats.nslots > initial_slots, "Map should have grown");
  REQUIRE_WITH_MSG(hmap_len(map) == 10, "Should have 10 elements after growth");
  // Remove elements that were inserted before growth
  hmap_remove(map, pre_growth_keys[1]);  // Remove key 2
  hmap_remove(map, pre_growth_keys[4]);  // Remove key 5
  REQUIRE_WITH_MSG(hmap_len(map) == 8, "Should have 8 elements after removals");
  REQUIRE_WITH_MSG(!hmap_contains(map, pre_growth_keys[1]),
                   "Pre-growth key 2 should be removed");
  REQUIRE_WITH_MSG(!hmap_contains(map, pre_growth_keys[4]),
                   "Pre-growth key 5 should be removed");
  // Remove elements that were inserted after growth
  hmap_remove(map, growth_keys[0]);  // Remove key 7
  hmap_remove(map, growth_keys[2]);  // Remove key 9
  REQUIRE_WITH_MSG(hmap_len(map) == 6, "Should have 6 elements after more removals");
  REQUIRE_WITH_MSG(!hmap_contains(map, growth_keys[0]),
                   "Post-growth key 7 should be removed");
  REQUIRE_WITH_MSG(!hmap_contains(map, growth_keys[2]),
                   "Post-growth key 9 should be removed");
  // Verify remaining elements are still accessible
  int remaining_keys[]   = {1, 3, 4, 6, 8, 10};
  int remaining_values[] = {10, 30, 40, 60, 80, 100};
  for (int i = 0; i < 6; i++) {
    REQUIRE_WITH_MSG(hmap_contains(map, remaining_keys[i]),
                     "Remaining key should be present");
    Entry* result = (Entry*)hmap_get(map, remaining_keys[i]);
    REQUIRE_WITH_MSG(result != NULL, "Should find remaining key");
    REQUIRE_WITH_MSG(result->value == remaining_values[i],
                     "Remaining key should have correct value");
  }
  hmap_free(map);
}

void test_hmap_remove_and_reinsert(void)
{
  typedef struct
  {
    int key;
    int value;
  } Entry;
  Entry* map = NULL;
  // Insert initial elements
  int key1 = 10, val1 = 100;
  int key2 = 20, val2 = 200;
  int key3 = 30, val3 = 300;
  hmap_put(map, key1, val1);
  hmap_put(map, key2, val2);
  hmap_put(map, key3, val3);
  HMapStats stats = _hmap_stats(map);
  REQUIRE_WITH_MSG(stats.n_used == 3, "Should have 3 used slots");
  REQUIRE_WITH_MSG(stats.n_removed == 0, "Should have 0 removed slots");
  // Remove middle element
  hmap_remove(map, key2);
  stats = _hmap_stats(map);
  REQUIRE_WITH_MSG(stats.n_used == 2, "Should have 2 used slots after removal");
  REQUIRE_WITH_MSG(stats.n_removed <= 1, "Should have at most 1 removed slot");
  REQUIRE_WITH_MSG(!hmap_contains(map, key2), "Key2 should not be present");
  // Reinsert the same key with different value
  int new_val2 = 999;
  hmap_put(map, key2, new_val2);
  stats = _hmap_stats(map);
  REQUIRE_WITH_MSG(hmap_len(map) == 3, "Should have 3 elements after reinsertion");
  REQUIRE_WITH_MSG(hmap_contains(map, key2), "Key2 should be present again");
  Entry* result = (Entry*)hmap_get(map, key2);
  REQUIRE_WITH_MSG(result != NULL, "Should find reinserted key");
  REQUIRE_WITH_MSG(result->value == new_val2, "Reinserted key should have new value");
  // Test multiple remove/reinsert cycles
  for (int cycle = 0; cycle < 3; cycle++) {
    hmap_remove(map, key1);
    REQUIRE_WITH_MSG(!hmap_contains(map, key1), "Key1 should be removed in cycle");
    int cycle_value = 1000 + cycle;
    hmap_put(map, key1, cycle_value);
    REQUIRE_WITH_MSG(hmap_contains(map, key1), "Key1 should be reinserted in cycle");
    result = (Entry*)hmap_get(map, key1);
    REQUIRE_WITH_MSG(result != NULL && result->value == cycle_value,
                     "Key1 should have cycle value");
  }
  // Verify other keys remain unaffected
  REQUIRE_WITH_MSG(hmap_contains(map, key3), "Key3 should still be present");
  result = (Entry*)hmap_get(map, key3);
  REQUIRE_WITH_MSG(result != NULL && result->value == val3,
                   "Key3 should have original value");
  hmap_free(map);
}

void test_hmap_remove_null_safety(void)
{
  typedef struct
  {
    int key;
    int value;
  } Entry;
  // Test remove on NULL map
  Entry* null_map = NULL;
  int    test_key = 42;
  hmap_remove(null_map, test_key);  // Should not crash
  REQUIRE_WITH_MSG(hmap_len(null_map) == 0, "NULL map should remain NULL/empty");
  // Test remove with extreme key values
  Entry* map              = NULL;
  int    extreme_keys[]   = {INT_MAX, INT_MIN, 0, -1};
  int    extreme_values[] = {1000, 2000, 3000, 4000};
  // Insert extreme values
  for (int i = 0; i < 4; i++) {
    hmap_put(map, extreme_keys[i], extreme_values[i]);
  }
  // Remove extreme values
  for (int i = 0; i < 4; i++) {
    REQUIRE_WITH_MSG(hmap_contains(map, extreme_keys[i]),
                     "Extreme key should be present before removal");
    hmap_remove(map, extreme_keys[i]);
    REQUIRE_WITH_MSG(!hmap_contains(map, extreme_keys[i]),
                     "Extreme key should be removed");
  }
  REQUIRE_WITH_MSG(hmap_len(map) == 0,
                   "Map should be empty after removing all extreme values");
  hmap_free(map);
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
  Entry8* map = NULL;
  // Test with int8_t range
  for (int8_t i = -50; i < 50; i++) {
    hmap_put(map, i, (int8_t)(i * 2));
  }
  REQUIRE_WITH_MSG(hmap_len(map) == 100, "Should contain 100 int8 entries");
  // Verify all entries
  for (int8_t i = -50; i < 50; i++) {
    REQUIRE_WITH_MSG(hmap_contains(map, i), "Should contain int8 key");
    Entry8* entry = (Entry8*)hmap_get(map, i);
    REQUIRE_WITH_MSG(entry != NULL && entry->value == i * 2,
                     "Should have correct int8 value");
  }
  hmap_free(map);
}

void test_hmap_data_types_int64(void)
{
  typedef struct
  {
    int64_t key;
    int64_t value;
  } Entry64;
  Entry64* map = NULL;
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
    hmap_put(map, test_keys[i], test_values[i]);
  }
  REQUIRE_WITH_MSG(hmap_len(map) == num_tests, "Should contain all int64 entries");
  for (size_t i = 0; i < num_tests; i++) {
    REQUIRE_WITH_MSG(hmap_contains(map, test_keys[i]), "Should contain int64 key");
    Entry64* entry = (Entry64*)hmap_get(map, test_keys[i]);
    REQUIRE_WITH_MSG(entry != NULL && entry->value == test_values[i],
                     "Should have correct int64 value");
  }
  hmap_free(map);
}

void test_hmap_data_types_float(void)
{
  typedef struct
  {
    float key;
    float value;
  } FloatEntry;
  FloatEntry* map         = NULL;
  float       test_keys[] = {-3.14159f, -1.0f, -0.5f, 0.0f, 0.5f, 1.0f, 2.71828f, 100.5f};
  float       test_values[] = {1.1f, 2.2f, 3.3f, 4.4f, 5.5f, 6.6f, 7.7f, 8.8f};
  size_t      num_tests     = sizeof(test_keys) / sizeof(test_keys[0]);
  for (size_t i = 0; i < num_tests; i++) {
    hmap_put(map, test_keys[i], test_values[i]);
  }
  REQUIRE_WITH_MSG(hmap_len(map) == num_tests, "Should contain all float entries");
  for (size_t i = 0; i < num_tests; i++) {
    REQUIRE_WITH_MSG(hmap_contains(map, test_keys[i]), "Should contain float key");
    FloatEntry* entry = (FloatEntry*)hmap_get(map, test_keys[i]);
    REQUIRE_WITH_MSG(entry != NULL && entry->value == test_values[i],
                     "Should have correct float value");
  }
  hmap_free(map);
}

void test_hmap_data_types_double(void)
{
  typedef struct
  {
    double key;
    double value;
  } DoubleEntry;
  DoubleEntry* map   = NULL;
  double test_keys[] = {-3.141592653589793, -1e-10, 0.0, 1e-10, 2.718281828459045, 1e10};
  double test_values[] = {
    1.123456789, 2.987654321, 3.456789012, 4.321098765, 5.678901234, 6.543210987};
  size_t num_tests = sizeof(test_keys) / sizeof(test_keys[0]);
  for (size_t i = 0; i < num_tests; i++) {
    hmap_put(map, test_keys[i], test_values[i]);
  }
  REQUIRE_WITH_MSG(hmap_len(map) == num_tests, "Should contain all double entries");
  for (size_t i = 0; i < num_tests; i++) {
    REQUIRE_WITH_MSG(hmap_contains(map, test_keys[i]), "Should contain double key");
    DoubleEntry* entry = (DoubleEntry*)hmap_get(map, test_keys[i]);
    REQUIRE_WITH_MSG(entry != NULL && entry->value == test_values[i],
                     "Should have correct double value");
  }
  hmap_free(map);
}

// Compaction stress test - force multiple compactions
void test_hmap_compaction_stress(void)
{
  typedef struct
  {
    int key;
    int value;
  } Entry;
  Entry*    map            = NULL;
  const int NUM_CYCLES     = 5;
  const int KEYS_PER_CYCLE = 100;
  for (int cycle = 0; cycle < NUM_CYCLES; cycle++) {
    // Insert many keys
    for (int i = 0; i < KEYS_PER_CYCLE; i++) {
      int key = cycle * KEYS_PER_CYCLE + i;
      hmap_put(map, key, key * 10);
    }
    HMapStats stats               = _hmap_stats(map);
    size_t    used_before_removal = stats.n_used;
    // Remove every 3rd key to create tombstones
    for (int i = 0; i < KEYS_PER_CYCLE; i += 3) {
      int key = cycle * KEYS_PER_CYCLE + i;
      hmap_remove(map, key);
    }
    stats = _hmap_stats(map);
    // Verify correct number of elements remain
    size_t expected_remaining =
      used_before_removal -
      (size_t)(KEYS_PER_CYCLE / 3 + (KEYS_PER_CYCLE % 3 > 0 ? 1 : 0));
    REQUIRE_WITH_MSG(stats.n_used == expected_remaining,
                     "Should have correct number of used slots after removal");
    // Verify all non-removed keys still exist
    for (int i = 0; i < KEYS_PER_CYCLE; i++) {
      int  key          = cycle * KEYS_PER_CYCLE + i;
      bool should_exist = (i % 3) != 0;
      if (should_exist) {
        REQUIRE_WITH_MSG(hmap_contains(map, key), "Non-removed key should still exist");
        Entry* entry = (Entry*)hmap_get(map, key);
        REQUIRE_WITH_MSG(entry != NULL && entry->value == key * 10,
                         "Should have correct value");
      }
      else {
        REQUIRE_WITH_MSG(!hmap_contains(map, key), "Removed key should not exist");
      }
    }
  }
  hmap_free(map);
}

// Hash collision stress test - keys designed to have similar hash values
void test_hmap_hash_collision_stress(void)
{
  typedef struct
  {
    int key;
    int value;
  } Entry;
  Entry*    map            = NULL;
  const int NUM_GROUPS     = 20;
  const int KEYS_PER_GROUP = 50;
  // Create keys that are likely to collide by using similar bit patterns
  for (int group = 0; group < NUM_GROUPS; group++) {
    int base_key = group * 1000000;  // Large spacing to create different hash groups
    for (int i = 0; i < KEYS_PER_GROUP; i++) {
      // Create keys with small variations that might hash to same bucket
      int key = base_key + (i * 7) + (i * i);  // Non-linear progression
      hmap_put(map, key, key + group);
    }
  }
  size_t total_keys = NUM_GROUPS * KEYS_PER_GROUP;
  REQUIRE_WITH_MSG(hmap_len(map) == total_keys, "Should contain all collision test keys");
  // Verify all keys and values
  for (int group = 0; group < NUM_GROUPS; group++) {
    int base_key = group * 1000000;
    for (int i = 0; i < KEYS_PER_GROUP; i++) {
      int key = base_key + (i * 7) + (i * i);
      REQUIRE_WITH_MSG(hmap_contains(map, key), "Should contain collision test key");
      Entry* entry = (Entry*)hmap_get(map, key);
      REQUIRE_WITH_MSG(entry != NULL && entry->value == key + group,
                       "Should have correct collision test value");
    }
  }
  // Remove half the keys to test collision handling with tombstones
  for (int group = 0; group < NUM_GROUPS; group += 2) {
    int base_key = group * 1000000;
    for (int i = 0; i < KEYS_PER_GROUP; i += 2) {
      int key = base_key + (i * 7) + (i * i);
      hmap_remove(map, key);
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
        REQUIRE_WITH_MSG(hmap_contains(map, key), "Remaining collision key should exist");
        Entry* entry = (Entry*)hmap_get(map, key);
        REQUIRE_WITH_MSG(entry != NULL && entry->value == key + group,
                         "Should have correct remaining value");
      }
      else {
        REQUIRE_WITH_MSG(!hmap_contains(map, key),
                         "Removed collision key should not exist");
      }
    }
  }
  hmap_free(map);
}

// Large-scale performance and correctness test
void test_hmap_large_scale_operations(void)
{
  typedef struct
  {
    int key;
    int value;
  } Entry;
  Entry*    map        = NULL;
  const int LARGE_SIZE = 10000;
  // Phase 1: Insert large number of sequential keys
  for (int i = 0; i < LARGE_SIZE; i++) {
    hmap_put(map, i, i * 3 + 7);
  }
  REQUIRE_WITH_MSG(hmap_len(map) == LARGE_SIZE, "Should contain all large-scale entries");
  HMapStats stats = _hmap_stats(map);
  REQUIRE_WITH_MSG(stats.n_used == LARGE_SIZE,
                   "Should have correct number of used slots");
  REQUIRE_WITH_MSG(stats.n_removed == 0, "Should have no removed slots after insertion");
  // Phase 2: Verify all entries
  for (int i = 0; i < LARGE_SIZE; i++) {
    REQUIRE_WITH_MSG(hmap_contains(map, i), "Should contain large-scale key");
    Entry* entry = (Entry*)hmap_get(map, i);
    REQUIRE_WITH_MSG(entry != NULL && entry->value == i * 3 + 7,
                     "Should have correct large-scale value");
  }
  // Phase 3: Remove every 5th element
  int removed_count = 0;
  for (int i = 0; i < LARGE_SIZE; i += 5) {
    hmap_remove(map, i);
    removed_count++;
  }
  REQUIRE_WITH_MSG(hmap_len(map) == (size_t)(LARGE_SIZE - removed_count),
                   "Should have correct count after large removals");
  // Phase 4: Verify state after removals
  for (int i = 0; i < LARGE_SIZE; i++) {
    bool should_exist = (i % 5) != 0;
    if (should_exist) {
      REQUIRE_WITH_MSG(hmap_contains(map, i), "Non-removed large-scale key should exist");
      Entry* entry = (Entry*)hmap_get(map, i);
      REQUIRE_WITH_MSG(entry != NULL && entry->value == i * 3 + 7,
                       "Should have correct remaining large-scale value");
    }
    else {
      REQUIRE_WITH_MSG(!hmap_contains(map, i),
                       "Removed large-scale key should not exist");
    }
  }
  // Phase 5: Re-insert removed keys with different values
  for (int i = 0; i < LARGE_SIZE; i += 5) {
    hmap_put(map, i, i * 5 + 11);  // Different value calculation
  }
  REQUIRE_WITH_MSG(hmap_len(map) == LARGE_SIZE,
                   "Should be back to full size after re-insertion");
  // Phase 6: Final verification with mixed values
  for (int i = 0; i < LARGE_SIZE; i++) {
    REQUIRE_WITH_MSG(hmap_contains(map, i), "Should contain all keys after re-insertion");
    Entry* entry = (Entry*)hmap_get(map, i);
    REQUIRE_WITH_MSG(entry != NULL, "Should find all keys after re-insertion");
    int expected_value = (i % 5 == 0) ? (i * 5 + 11) : (i * 3 + 7);
    REQUIRE_WITH_MSG(entry->value == expected_value,
                     "Should have correct value after mixed operations");
  }
  hmap_free(map);
}

// Mixed operation patterns test
void test_hmap_mixed_operation_patterns(void)
{
  typedef struct
  {
    int key;
    int value;
  } Entry;
  Entry*    map          = NULL;
  const int PATTERN_SIZE = 1000;
  // Pattern 1: Alternating insert/remove
  for (int i = 0; i < PATTERN_SIZE; i++) {
    hmap_put(map, i, i * 2);
    if (i > 0 && (i % 3) == 0) {
      int prev_key = i - 2;
      hmap_remove(map, prev_key);  // Remove an earlier key
    }
  }
  // Pattern 2: Batch operations
  for (int batch = 0; batch < 5; batch++) {
    int batch_start = PATTERN_SIZE + batch * 100;
    // Insert batch
    for (int i = 0; i < 100; i++) {
      int key   = batch_start + i;
      int value = key * 3;
      hmap_put(map, key, value);
    }
    // Update some values in batch
    for (int i = 0; i < 100; i += 4) {
      int key   = batch_start + i;
      int value = key * 4;  // Different multiplier
      hmap_put(map, key, value);
    }
    // Remove some from batch
    for (int i = 1; i < 100; i += 4) {
      int key = batch_start + i;
      hmap_remove(map, key);
    }
  }
  // Pattern 3: Verify the complex state
  size_t final_len = hmap_len(map);
  REQUIRE_WITH_MSG(final_len > 0, "Should have elements after mixed operations");
  HMapStats final_stats = _hmap_stats(map);
  REQUIRE_WITH_MSG(final_stats.n_used == final_len, "Used count should match length");
  // Ensure all contained keys are retrievable and have correct values
  for (int i = 0; i < PATTERN_SIZE + 500; i++) {
    if (hmap_contains(map, i)) {
      Entry* entry = (Entry*)hmap_get(map, i);
      REQUIRE_WITH_MSG(entry != NULL, "Contained key should be retrievable");
      REQUIRE_WITH_MSG(entry->key == i, "Key should match");
      // Value correctness depends on the complex pattern, just ensure it's not garbage
      REQUIRE_WITH_MSG(entry->value != 0 || i == 0, "Value should be meaningful");
    }
  }
  hmap_free(map);
}

void test_hmap_header_alignment(void)
{
  REQUIRE_WITH_MSG(
    sizeof(_HashTableHeader) % sizeof(_MaxAlignCompat) == 0,
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
  Entry* map = NULL;
  // Test empty map
  REQUIRE_WITH_MSG(hmap_is_empty(map), "NULL map should be empty");
  REQUIRE_WITH_MSG(hmap_len(map) == 0, "NULL map should have length 0");
  // Test after adding element
  int key1 = 42, val1 = 100;
  hmap_put(map, key1, val1);
  REQUIRE_WITH_MSG(!hmap_is_empty(map), "Map with element should not be empty");
  REQUIRE_WITH_MSG(hmap_len(map) == 1, "Map should have length 1");
  // Test after removing element
  hmap_remove(map, key1);
  REQUIRE_WITH_MSG(hmap_is_empty(map), "Map should be empty after removing only element");
  REQUIRE_WITH_MSG(hmap_len(map) == 0, "Map should have length 0 after removal");
  // Test with multiple elements
  for (int i = 0; i < 10; i++) {
    int key = i, val = i * 2;
    hmap_put(map, key, val);
    REQUIRE_WITH_MSG(!hmap_is_empty(map), "Map should not be empty during population");
  }
  // Remove all elements
  for (int i = 0; i < 10; i++) {
    int key = i;
    hmap_remove(map, key);
  }
  REQUIRE_WITH_MSG(hmap_is_empty(map), "Map should be empty after removing all elements");
  hmap_free(map);
}

void test_hmap_iterate_after_remove(void)
{
  typedef struct
  {
    int    key;
    double value;
  } Entry;
  Entry* map = NULL;
  // Test 1: Empty map.
  REQUIRE_WITH_MSG(hmap_is_empty(map), "NULL map should be empty");
  REQUIRE_WITH_MSG(hmap_len(map) == 0, "NULL map should have length 0");
  // Test 2: Insert 10 entries, remove from the middle, check compactness.
  for (int key = 0; key < 10; ++key) {
    hmap_put(map, key, sin(0.2 * (double)key));
  }
  REQUIRE_WITH_MSG(hmap_len(map) == 10, "Map should have 10 entries");
  {
    HMapStats stats = _hmap_stats(map);
    REQUIRE_WITH_MSG(stats.n_used == 10, "Should have 10 used slots after insert");
    REQUIRE_WITH_MSG(stats.n_removed == 0, "No removed slots after fresh inserts");
    REQUIRE_WITH_MSG(stats.nslots >= 10, "Must have enough slots for 10 entries");
    REQUIRE_WITH_MSG(stats.nbuckets == stats.nslots / HMAP_BUCKET_SIZE,
                     "Bucket count must match slot count");
  }
  for (size_t i = 0; i < hmap_len(map); ++i) {
    Entry* e = hmap_get(map, map[i].key);
    REQUIRE_WITH_MSG(e != NULL && e >= map && e < map + hmap_len(map),
                     "Every entry must be findable within [0..len)");
  }
  // Test 3: Remove from the middle.
  int remove_key = 3;
  hmap_remove(map, remove_key);
  REQUIRE_WITH_MSG(!hmap_contains(map, remove_key), "Removed key must not be found");
  REQUIRE_WITH_MSG(hmap_len(map) == 9, "Length must decrease");
  {
    HMapStats stats = _hmap_stats(map);
    REQUIRE_WITH_MSG(stats.n_used == 9, "Should have 9 used slots after removing key 3");
    REQUIRE_WITH_MSG(stats.n_removed <= 1, "At most 1 tombstone after single remove");
  }
  for (size_t i = 0; i < hmap_len(map); ++i) {
    Entry* e = hmap_get(map, map[i].key);
    REQUIRE_WITH_MSG(e != NULL && e >= map && e < map + hmap_len(map),
                     "Compact invariant after removing key 3");
  }
  // Test 4: Remove another.
  remove_key = 5;
  hmap_remove(map, remove_key);
  REQUIRE_WITH_MSG(!hmap_contains(map, remove_key), "Removed key must not be found");
  REQUIRE_WITH_MSG(hmap_len(map) == 8, "Length must decrease");
  {
    HMapStats stats = _hmap_stats(map);
    REQUIRE_WITH_MSG(stats.n_used == 8, "Should have 8 used slots after removing key 5");
    REQUIRE_WITH_MSG(stats.n_removed <= 2, "At most 2 tombstones after two removes");
  }
  for (size_t i = 0; i < hmap_len(map); ++i) {
    Entry* e = hmap_get(map, map[i].key);
    REQUIRE_WITH_MSG(e != NULL && e >= map && e < map + hmap_len(map),
                     "Compact invariant after removing key 5");
  }
  // Test 5: Remove the first key (swap with last).
  remove_key = 0;
  hmap_remove(map, remove_key);
  REQUIRE_WITH_MSG(!hmap_contains(map, remove_key), "Removed key 0 must not be found");
  for (size_t i = 0; i < hmap_len(map); ++i) {
    Entry* e = hmap_get(map, map[i].key);
    REQUIRE_WITH_MSG(e != NULL && e >= map && e < map + hmap_len(map),
                     "Compact invariant after removing key 0");
  }
  // Test 6: Remove the last key (no swap needed).
  remove_key = 9;
  hmap_remove(map, remove_key);
  REQUIRE_WITH_MSG(!hmap_contains(map, remove_key), "Removed key 9 must not be found");
  for (size_t i = 0; i < hmap_len(map); ++i) {
    Entry* e = hmap_get(map, map[i].key);
    REQUIRE_WITH_MSG(e != NULL && e >= map && e < map + hmap_len(map),
                     "Compact invariant after removing key 9");
  }
  // Test 7: Remove nonexistent key is a no-op.
  {
    size_t const before = hmap_len(map);
    remove_key          = 999;
    hmap_remove(map, remove_key);
    REQUIRE_WITH_MSG(hmap_len(map) == before, "Removing nonexistent key is a no-op");
  }
  // Test 8: Remove all remaining one by one.
  while (!hmap_is_empty(map)) {
    int key_to_remove = map[0].key;
    hmap_remove(map, key_to_remove);
    REQUIRE_WITH_MSG(!hmap_contains(map, key_to_remove),
                     "Just-removed key must not exist");
    for (size_t i = 0; i < hmap_len(map); ++i) {
      Entry* e = hmap_get(map, map[i].key);
      REQUIRE_WITH_MSG(e != NULL && e >= map && e < map + hmap_len(map),
                       "Compact invariant while draining map");
    }
  }
  REQUIRE_WITH_MSG(hmap_len(map) == 0, "Map should be empty after removing all");
  {
    HMapStats stats = _hmap_stats(map);
    REQUIRE_WITH_MSG(stats.n_used == 0, "Should have 0 used slots when fully drained");
    REQUIRE_WITH_MSG(stats.n_removed <= 10,
                     "Tombstones bounded by original size (may compact)");
  }
  hmap_free(map);
  // Test 9: Insert, remove evens, re-insert (exercises swap-remove + growth).
  map = NULL;
  for (int key = 0; key < 50; ++key) {
    hmap_put(map, key, (double)key);
  }
  for (int key = 0; key < 50; key += 2) {
    hmap_remove(map, key);
  }
  REQUIRE_WITH_MSG(hmap_len(map) == 25, "25 entries after removing evens");
  {
    HMapStats stats = _hmap_stats(map);
    REQUIRE_WITH_MSG(stats.n_used == 25,
                     "Should have 25 used slots after removing evens");
    REQUIRE_WITH_MSG(stats.nslots >= 25, "Must have enough slots for 25 entries");
  }
  for (size_t i = 0; i < hmap_len(map); ++i) {
    Entry* e = hmap_get(map, map[i].key);
    REQUIRE_WITH_MSG(e != NULL && e >= map && e < map + hmap_len(map),
                     "Compact invariant after removing evens");
  }
  for (int key = 0; key < 50; key += 2) {
    hmap_put(map, key, (double)(key * 10));
  }
  REQUIRE_WITH_MSG(hmap_len(map) == 50, "50 entries after re-insert");
  {
    HMapStats stats = _hmap_stats(map);
    REQUIRE_WITH_MSG(stats.n_used == 50, "Should have 50 used slots after re-insert");
    REQUIRE_WITH_MSG(stats.nslots >= 50, "Must have enough slots for 50 entries");
    REQUIRE_WITH_MSG(stats.nbuckets == stats.nslots / HMAP_BUCKET_SIZE,
                     "Bucket count must match after re-insert");
  }
  for (int key = 0; key < 50; ++key) {
    Entry* e = hmap_get(map, key);
    REQUIRE_WITH_MSG(e != NULL, "All 50 keys must exist");
    double expected = (key % 2 == 0) ? (double)(key * 10) : (double)key;
    REQUIRE_WITH_MSG(e->value == expected, "Value must match after re-insert");
    REQUIRE_WITH_MSG(e >= map && e < map + hmap_len(map),
                     "Compact invariant after re-insert");
  }
  hmap_free(map);
  // Test 10: Many removes triggering compaction (n_removed > n_total/4).
  map = NULL;
  for (int key = 0; key < 100; ++key) {
    hmap_put(map, key, (double)key);
  }
  for (int key = 0; key < 80; ++key) {
    hmap_remove(map, key);
    for (size_t i = 0; i < hmap_len(map); ++i) {
      Entry* e = hmap_get(map, map[i].key);
      REQUIRE_WITH_MSG(e != NULL && e >= map && e < map + hmap_len(map),
                       "Compact invariant during bulk remove");
    }
  }
  REQUIRE_WITH_MSG(hmap_len(map) == 20, "20 entries left");
  {
    HMapStats stats = _hmap_stats(map);
    REQUIRE_WITH_MSG(stats.n_used == 20, "Should have 20 used slots after bulk remove");
    REQUIRE_WITH_MSG(stats.nslots >= 20, "Must have enough slots for remaining entries");
  }
  for (size_t i = 0; i < hmap_len(map); ++i) {
    REQUIRE_WITH_MSG(map[i].key >= 80 && map[i].key < 100,
                     "Remaining keys must be in [80, 100)");
  }
  hmap_free(map);
  // Test 11: Single element insert and remove.
  map = NULL;
  {
    int k = 42;
    hmap_put(map, k, 3.14);
    REQUIRE_WITH_MSG(hmap_len(map) == 1, "Single element map");
    Entry* e = hmap_get(map, k);
    REQUIRE_WITH_MSG(e != NULL && e >= map && e < map + hmap_len(map),
                     "Single element compact invariant");
    hmap_remove(map, k);
    REQUIRE_WITH_MSG(hmap_len(map) == 0, "Empty after single remove");
    {
      HMapStats stats = _hmap_stats(map);
      REQUIRE_WITH_MSG(stats.n_used == 0, "Should have 0 used slots after single remove");
      REQUIRE_WITH_MSG(stats.n_removed <= 1, "At most 1 tombstone after single remove");
    }
  }
  hmap_free(map);
}

// Hash Set Tests

void test_hset_basic_operations(void)
{
  int* set = NULL;
  // Test empty set
  REQUIRE_WITH_MSG(hset_len(set) == 0, "New set should be empty");
  REQUIRE_WITH_MSG(hset_is_empty(set), "New set should report as empty");
  // Test adding elements
  int val1 = 42;
  hset_put(set, val1);
  REQUIRE_WITH_MSG(hset_len(set) == 1, "Set should have 1 element");
  REQUIRE_WITH_MSG(!hset_is_empty(set), "Set should not be empty");
  REQUIRE_WITH_MSG(hset_contains(set, val1), "Set should contain added element");
  // Test adding duplicate (should not increase size)
  hset_put(set, val1);
  REQUIRE_WITH_MSG(hset_len(set) == 1, "Set should still have 1 element after duplicate");
  REQUIRE_WITH_MSG(hset_contains(set, val1), "Set should still contain element");
  // Test adding different elements
  int val2 = 10, val3 = 20, val4 = 30;
  hset_put(set, val2);
  hset_put(set, val3);
  hset_put(set, val4);
  REQUIRE_WITH_MSG(hset_len(set) == 4, "Set should have 4 unique elements");
  // Verify all elements are present
  REQUIRE_WITH_MSG(hset_contains(set, val1), "Set should contain 42");
  REQUIRE_WITH_MSG(hset_contains(set, val2), "Set should contain 10");
  REQUIRE_WITH_MSG(hset_contains(set, val3), "Set should contain 20");
  REQUIRE_WITH_MSG(hset_contains(set, val4), "Set should contain 30");
  // Test non-existent elements
  int missing1 = 99, missing2 = -1;
  REQUIRE_WITH_MSG(!hset_contains(set, missing1), "Set should not contain 99");
  REQUIRE_WITH_MSG(!hset_contains(set, missing2), "Set should not contain -1");
  hset_free(set);
}

void test_hset_remove_operations(void)
{
  int* set = NULL;
  // Add some elements
  for (int i = 0; i < 10; i++) {
    hset_put(set, i);
  }
  REQUIRE_WITH_MSG(hset_len(set) == 10, "Set should have 10 elements");
  // Remove elements
  int remove_val = 5;
  hset_remove(set, remove_val);
  REQUIRE_WITH_MSG(hset_len(set) == 9, "Set should have 9 elements after removal");
  REQUIRE_WITH_MSG(!hset_contains(set, remove_val),
                   "Set should not contain removed element");
  // Remove non-existent element (should not crash or change size)
  int missing_val = 99;
  hset_remove(set, missing_val);
  REQUIRE_WITH_MSG(hset_len(set) == 9,
                   "Set size should not change when removing non-existent element");
  // Remove all elements
  for (int i = 0; i < 10; i++) {
    if (i != 5) {  // Skip already removed element
      hset_remove(set, i);
    }
  }
  REQUIRE_WITH_MSG(hset_is_empty(set), "Set should be empty after removing all elements");
  hset_free(set);
}

void test_hset_different_types(void)
{
  // Test with doubles
  double* dset = NULL;
  double  d1 = 3.14, d2 = 2.71;
  hset_put(dset, d1);
  hset_put(dset, d2);
  hset_put(dset, d1);  // Duplicate
  REQUIRE_WITH_MSG(hset_len(dset) == 2, "Double set should have 2 unique elements");
  REQUIRE_WITH_MSG(hset_contains(dset, d1), "Set should contain 3.14");
  REQUIRE_WITH_MSG(hset_contains(dset, d2), "Set should contain 2.71");
  hset_free(dset);
  // Test with character values (not strings)
  char* cset = NULL;
  char  c1 = 'a', c2 = 'b', c3 = 'a';  // Duplicate character
  hset_put(cset, c1);
  hset_put(cset, c2);
  hset_put(cset, c3);  // Should not increase size
  REQUIRE_WITH_MSG(hset_len(cset) == 2, "Character set should have 2 unique elements");
  REQUIRE_WITH_MSG(hset_contains(cset, c1), "Set should contain 'a'");
  REQUIRE_WITH_MSG(hset_contains(cset, c2), "Set should contain 'b'");
  hset_free(cset);
}

void test_hset_large_scale(void)
{
  int*      set  = NULL;
  const int SIZE = 1000;
  // Add many elements
  for (int i = 0; i < SIZE; i++) {
    hset_put(set, i);
  }
  REQUIRE_WITH_MSG(hset_len(set) == SIZE, "Set should contain all added elements");
  // Verify all elements are present
  for (int i = 0; i < SIZE; i++) {
    REQUIRE_WITH_MSG(hset_contains(set, i), "Set should contain all added elements");
  }
  // Add duplicates (should not change size)
  for (int i = 0; i < SIZE; i += 2) {
    hset_put(set, i);
  }
  REQUIRE_WITH_MSG(hset_len(set) == SIZE,
                   "Set size should not change after adding duplicates");
  // Remove half the elements
  for (int i = 0; i < SIZE; i += 2) {
    hset_remove(set, i);
  }
  REQUIRE_WITH_MSG(hset_len(set) == SIZE / 2,
                   "Set should have half the elements after removal");
  // Verify correct elements remain
  for (int i = 0; i < SIZE; i++) {
    if (i % 2 == 0) {
      REQUIRE_WITH_MSG(!hset_contains(set, i), "Even elements should be removed");
    }
    else {
      REQUIRE_WITH_MSG(hset_contains(set, i), "Odd elements should remain");
    }
  }
  hset_free(set);
}

void test_hset_edge_cases(void)
{
  int* set = NULL;
  // Test operations on NULL set
  int test_val = 42;
  REQUIRE_WITH_MSG(hset_is_empty(set), "NULL set should be empty");
  REQUIRE_WITH_MSG(hset_len(set) == 0, "NULL set should have length 0");
  REQUIRE_WITH_MSG(!hset_contains(set, test_val),
                   "NULL set should not contain any elements");
  // Test single element operations
  int single_val = 1;
  hset_put(set, single_val);
  REQUIRE_WITH_MSG(hset_len(set) == 1, "Set should have 1 element");
  REQUIRE_WITH_MSG(!hset_is_empty(set), "Set with element should not be empty");
  hset_remove(set, single_val);
  REQUIRE_WITH_MSG(hset_is_empty(set), "Set should be empty after removing only element");
  // Test zero value
  int zero_val = 0;
  hset_put(set, zero_val);
  REQUIRE_WITH_MSG(hset_contains(set, zero_val), "Set should be able to store zero");
  REQUIRE_WITH_MSG(hset_len(set) == 1, "Set with zero should have length 1");
  // Test negative values
  int neg1 = -42, neg2 = -1;
  hset_put(set, neg1);
  hset_put(set, neg2);
  REQUIRE_WITH_MSG(hset_contains(set, neg1), "Set should handle negative values");
  REQUIRE_WITH_MSG(hset_contains(set, neg2), "Set should handle negative values");
  REQUIRE_WITH_MSG(hset_len(set) == 3, "Set should have 3 elements (0, -42, -1)");
  hset_free(set);
}

void test_hset_memory_operations(void)
{
  int* set = NULL;
  // Test reserve functionality
  Status result = hset_reserve(set, 100);
  REQUIRE_WITH_MSG(result == OK, "Reserve should succeed");
  // Add elements after reserve
  for (int i = 0; i < 50; i++) {
    hset_put(set, i);
  }
  REQUIRE_WITH_MSG(hset_len(set) == 50, "Set should have 50 elements after population");
  // Test multiple free calls (should be safe)
  hset_free(set);
  set = NULL;
  hset_free(set);  // Should not crash
}

void test_hset_is_empty_comprehensive(void)
{
  int* set = NULL;
  // Test empty states
  REQUIRE_WITH_MSG(hset_is_empty(set), "NULL set should be empty");
  REQUIRE_WITH_MSG(hset_is_empty(NULL), "Explicit NULL should be empty");
  // Test after adding and removing
  int test_elem = 42;
  hset_put(set, test_elem);
  REQUIRE_WITH_MSG(!hset_is_empty(set), "Set with element should not be empty");
  hset_remove(set, test_elem);
  REQUIRE_WITH_MSG(hset_is_empty(set), "Set should be empty after removing only element");
  // Test with multiple add/remove cycles
  for (int cycle = 0; cycle < 5; cycle++) {
    for (int i = 0; i < 10; i++) {
      hset_put(set, i);
    }
    REQUIRE_WITH_MSG(!hset_is_empty(set), "Set should not be empty during population");
    for (int i = 0; i < 10; i++) {
      hset_remove(set, i);
    }
    REQUIRE_WITH_MSG(hset_is_empty(set), "Set should be empty after clearing");
  }
  hset_free(set);
}

// String Hashmap Tests

void test_hmap_str_basic(void)
{
  typedef struct
  {
    char const* key;
    int         value;
  } StrEntry;
  StrEntry* map = NULL;
  // Test empty map
  REQUIRE_WITH_MSG(hmap_len(map) == 0, "Empty string map should have length 0");
  HMapStats stats = _hmap_stats(map);
  REQUIRE_WITH_MSG(stats.n_used == 0, "Empty string map should have 0 used slots");
  REQUIRE_WITH_MSG(stats.nslots == 0, "Empty string map should have 0 total slots");
  REQUIRE_WITH_MSG(stats.nbuckets == 0, "Empty string map should have 0 buckets");
  // Test single insertion
  char const* key1 = "hello";
  int         val1 = 100;
  hmap_str_put(map, key1, val1);
  REQUIRE_WITH_MSG(hmap_len(map) == 1, "String map should have 1 element after insert");
  REQUIRE_WITH_MSG(map[0].key == key1, "First entry key should be 'hello'");
  REQUIRE_WITH_MSG(map[0].value == 100, "First entry value should be 100");
  stats = _hmap_stats(map);
  REQUIRE_WITH_MSG(stats.n_used == 1, "Should have 1 used slot after insert");
  REQUIRE_WITH_MSG(stats.nslots == HMAP_BUCKET_SIZE,
                   "Initial size should be HMAP_BUCKET_SIZE slots");
  REQUIRE_WITH_MSG(stats.nbuckets == 1, "Should have 1 bucket initially");
  REQUIRE_WITH_MSG(stats.n_removed == 0, "Should have no removed entries");
  // Test multiple insertions
  char const* key2 = "world";
  char const* key3 = "test";
  int         val2 = 200;
  int         val3 = 300;
  hmap_str_put(map, key2, val2);
  hmap_str_put(map, key3, val3);
  REQUIRE_WITH_MSG(hmap_len(map) == 3, "String map should have 3 elements");
  // Test duplicate key (should update value only)
  int val1_updated = 999;
  hmap_str_put(map, key1, val1_updated);
  REQUIRE_WITH_MSG(hmap_len(map) == 3, "Length should stay 3 after duplicate insert");
  // Find the updated entry
  bool found = false;
  for (size_t i = 0; i < hmap_len(map); i++) {
    if (map[i].key == key1) {
      REQUIRE_WITH_MSG(map[i].value == val1_updated, "Duplicate key should update value");
      found = true;
      break;
    }
  }
  REQUIRE_WITH_MSG(found, "Should find the updated entry");
  hmap_free(map);
}

void test_hmap_str_get_basic(void)
{
  typedef struct
  {
    char const* key;
    int         value;
  } StrEntry;
  StrEntry* map = NULL;
  // Test get from empty map
  char const* key    = "missing";
  StrEntry*   result = (StrEntry*)hmap_str_get(map, key);
  REQUIRE_WITH_MSG(result == NULL, "Get from empty string map should return NULL");
  REQUIRE_WITH_MSG(!hmap_str_contains(map, key),
                   "Empty string map should not contain any key");
  // Insert some entries
  char const* key1 = "apple";
  char const* key2 = "banana";
  char const* key3 = "cherry";
  int         val1 = 100;
  int         val2 = 200;
  int         val3 = 300;
  hmap_str_put(map, key1, val1);
  hmap_str_put(map, key2, val2);
  hmap_str_put(map, key3, val3);
  // Test hmap_str_contains
  REQUIRE_WITH_MSG(hmap_str_contains(map, key1), "Should contain 'apple'");
  REQUIRE_WITH_MSG(hmap_str_contains(map, key2), "Should contain 'banana'");
  REQUIRE_WITH_MSG(hmap_str_contains(map, key3), "Should contain 'cherry'");
  char const* missing_key1 = "orange";
  char const* missing_key2 = "grape";
  REQUIRE_WITH_MSG(!hmap_str_contains(map, missing_key1), "Should not contain 'orange'");
  REQUIRE_WITH_MSG(!hmap_str_contains(map, missing_key2), "Should not contain 'grape'");
  // Test hmap_str_get for existing keys
  result = (StrEntry*)hmap_str_get(map, key1);
  REQUIRE_WITH_MSG(result != NULL, "Should find 'apple'");
  REQUIRE_WITH_MSG(result->key == key1, "Retrieved key should match");
  REQUIRE_WITH_MSG(result->value == val1, "Retrieved value should match");
  result = (StrEntry*)hmap_str_get(map, key2);
  REQUIRE_WITH_MSG(result != NULL, "Should find 'banana'");
  REQUIRE_WITH_MSG(result->value == val2, "Retrieved value should match");
  result = (StrEntry*)hmap_str_get(map, key3);
  REQUIRE_WITH_MSG(result != NULL, "Should find 'cherry'");
  REQUIRE_WITH_MSG(result->value == val3, "Retrieved value should match");
  // Test hmap_str_get for missing keys
  result = (StrEntry*)hmap_str_get(map, missing_key1);
  REQUIRE_WITH_MSG(result == NULL, "Should not find 'orange'");
  result = (StrEntry*)hmap_str_get(map, missing_key2);
  REQUIRE_WITH_MSG(result == NULL, "Should not find 'grape'");
  hmap_free(map);
}

void test_hmap_str_remove_basic(void)
{
  typedef struct
  {
    char const* key;
    int         value;
  } StrEntry;
  StrEntry* map = NULL;
  // Remove from empty map (should be safe)
  char const* key = "missing";
  hmap_str_remove(map, key);
  REQUIRE_WITH_MSG(hmap_len(map) == 0, "Remove from empty map should be safe");
  // Insert some entries
  char const* key1 = "dog";
  char const* key2 = "cat";
  char const* key3 = "bird";
  int         val1 = 1;
  int         val2 = 2;
  int         val3 = 3;
  hmap_str_put(map, key1, val1);
  hmap_str_put(map, key2, val2);
  hmap_str_put(map, key3, val3);
  REQUIRE_WITH_MSG(hmap_len(map) == 3, "Should have 3 entries before removal");
  // Remove middle entry
  hmap_str_remove(map, key2);
  REQUIRE_WITH_MSG(hmap_len(map) == 2, "Should have 2 entries after removal");
  REQUIRE_WITH_MSG(!hmap_str_contains(map, key2), "Should not contain removed key");
  REQUIRE_WITH_MSG(hmap_str_contains(map, key1), "Should still contain key1");
  REQUIRE_WITH_MSG(hmap_str_contains(map, key3), "Should still contain key3");
  // Remove non-existent key (should be safe)
  char const* missing_key = "fish";
  hmap_str_remove(map, missing_key);
  REQUIRE_WITH_MSG(hmap_len(map) == 2, "Length should stay 2 after removing missing key");
  // Remove remaining entries
  hmap_str_remove(map, key1);
  REQUIRE_WITH_MSG(hmap_len(map) == 1, "Should have 1 entry after second removal");
  REQUIRE_WITH_MSG(!hmap_str_contains(map, key1), "Should not contain removed key1");
  REQUIRE_WITH_MSG(hmap_str_contains(map, key3), "Should still contain key3");
  hmap_str_remove(map, key3);
  REQUIRE_WITH_MSG(hmap_len(map) == 0, "Should have 0 entries after final removal");
  REQUIRE_WITH_MSG(!hmap_str_contains(map, key3), "Should not contain removed key3");
  hmap_free(map);
}

void test_hmap_str_edge_cases(void)
{
  typedef struct
  {
    char const* key;
    int         value;
  } StrEntry;
  StrEntry* map = NULL;
  // Test empty string key
  char const* empty_key = "";
  int         val       = 42;
  hmap_str_put(map, empty_key, val);
  REQUIRE_WITH_MSG(hmap_len(map) == 1, "Should handle empty string key");
  REQUIRE_WITH_MSG(hmap_str_contains(map, empty_key), "Should contain empty string key");
  StrEntry* result = (StrEntry*)hmap_str_get(map, empty_key);
  REQUIRE_WITH_MSG(result != NULL, "Should find empty string key");
  REQUIRE_WITH_MSG(result->value == val, "Should retrieve correct value for empty key");
  // Test very long string key
  char const* long_key =
    "this_is_a_very_long_string_key_that_should_test_hash_function_behavior_with_long_"
    "inputs_and_make_sure_everything_works_correctly_even_with_extended_key_lengths";
  int long_val = 999;
  hmap_str_put(map, long_key, long_val);
  REQUIRE_WITH_MSG(hmap_len(map) == 2, "Should handle very long string key");
  REQUIRE_WITH_MSG(hmap_str_contains(map, long_key), "Should contain long string key");
  result = (StrEntry*)hmap_str_get(map, long_key);
  REQUIRE_WITH_MSG(result != NULL, "Should find long string key");
  REQUIRE_WITH_MSG(result->value == long_val,
                   "Should retrieve correct value for long key");
  // Test similar keys (potential hash collisions)
  char const* key1 = "test123";
  char const* key2 = "test124";
  char const* key3 = "test125";
  hmap_str_put(map, key1, 1);
  hmap_str_put(map, key2, 2);
  hmap_str_put(map, key3, 3);
  REQUIRE_WITH_MSG(hmap_len(map) == 5, "Should handle similar keys");
  REQUIRE_WITH_MSG(hmap_str_contains(map, key1), "Should contain key1");
  REQUIRE_WITH_MSG(hmap_str_contains(map, key2), "Should contain key2");
  REQUIRE_WITH_MSG(hmap_str_contains(map, key3), "Should contain key3");
  // Verify values are distinct
  result = (StrEntry*)hmap_str_get(map, key1);
  REQUIRE_WITH_MSG(result->value == 1, "key1 should have value 1");
  result = (StrEntry*)hmap_str_get(map, key2);
  REQUIRE_WITH_MSG(result->value == 2, "key2 should have value 2");
  result = (StrEntry*)hmap_str_get(map, key3);
  REQUIRE_WITH_MSG(result->value == 3, "key3 should have value 3");
  hmap_free(map);
}

void test_hmap_str_stress_test(void)
{
  typedef struct
  {
    char const* key;
    int         value;
  } StrEntry;
  StrEntry* map = NULL;
  // Create test keys
  char const* keys[] = {
    "alpha",    "beta",     "gamma",    "delta",    "epsilon",  "zeta",    "eta",
    "theta",    "iota",     "kappa",    "lambda",   "mu",       "nu",      "xi",
    "omicron",  "pi",       "rho",      "sigma",    "tau",      "upsilon", "phi",
    "chi",      "psi",      "omega",    "zero",     "one",      "two",     "three",
    "four",     "five",     "six",      "seven",    "eight",    "nine",    "ten",
    "eleven",   "twelve",   "thirteen", "fourteen", "fifteen",  "sixteen", "seventeen",
    "eighteen", "nineteen", "twenty",   "hundred",  "thousand", "million", "billion"};
  size_t num_keys = sizeof(keys) / sizeof(keys[0]);
  // Insert all keys
  for (size_t i = 0; i < num_keys; i++) {
    hmap_str_put(map, keys[i], (int)i);
  }
  REQUIRE_WITH_MSG(hmap_len(map) == num_keys, "Should contain all inserted keys");
  // Verify all keys can be found
  for (size_t i = 0; i < num_keys; i++) {
    REQUIRE_WITH_MSG(hmap_str_contains(map, keys[i]), "Should contain each key");
    StrEntry* result = (StrEntry*)hmap_str_get(map, keys[i]);
    REQUIRE_WITH_MSG(result != NULL, "Should find each key");
    REQUIRE_WITH_MSG(result->value == (int)i, "Should have correct value");
  }
  // Remove every other key
  for (size_t i = 0; i < num_keys; i += 2) {
    hmap_str_remove(map, keys[i]);
  }
  size_t expected_remaining = num_keys - (num_keys + 1) / 2;
  REQUIRE_WITH_MSG(hmap_len(map) == expected_remaining,
                   "Should have correct count after removals");
  // Verify remaining keys
  for (size_t i = 1; i < num_keys; i += 2) {
    REQUIRE_WITH_MSG(hmap_str_contains(map, keys[i]), "Odd keys should remain");
  }
  // Verify removed keys
  for (size_t i = 0; i < num_keys; i += 2) {
    REQUIRE_WITH_MSG(!hmap_str_contains(map, keys[i]), "Even keys should be removed");
  }
  hmap_free(map);
}

void test_hmap_str_growth_and_compaction(void)
{
  typedef struct
  {
    char const* key;
    int         value;
  } StrEntry;
  StrEntry* map = NULL;
  // Force multiple growth cycles
  char keys[100][20];
  for (int i = 0; i < 100; i++) {
    snprintf(keys[i], sizeof(keys[i]), "key_%d", i);
    hmap_str_put(map, keys[i], i);
  }
  REQUIRE_WITH_MSG(hmap_len(map) == 100, "Should handle growth to 100 elements");
  // Verify all keys
  for (int i = 0; i < 100; i++) {
    REQUIRE_WITH_MSG(hmap_str_contains(map, keys[i]),
                     "Should contain all keys after growth");
  }
  // Remove many keys to trigger compaction
  for (int i = 0; i < 80; i++) {
    hmap_str_remove(map, keys[i]);
  }
  REQUIRE_WITH_MSG(hmap_len(map) == 20, "Should have 20 keys after removal");
  // Verify remaining keys still work
  for (int i = 80; i < 100; i++) {
    REQUIRE_WITH_MSG(hmap_str_contains(map, keys[i]),
                     "Remaining keys should be accessible");
    StrEntry* result = (StrEntry*)hmap_str_get(map, keys[i]);
    REQUIRE_WITH_MSG(result != NULL, "Should find remaining keys");
    REQUIRE_WITH_MSG(result->value == i, "Should have correct values");
  }
  hmap_free(map);
}

// String Hashset Tests

void test_hset_str_basic_operations(void)
{
  char const** set = NULL;
  // Test empty set
  REQUIRE_WITH_MSG(hset_len(set) == 0, "New string set should be empty");
  REQUIRE_WITH_MSG(hset_is_empty(set), "New string set should report as empty");
  // Test adding elements
  char const* val1 = "hello";
  hset_str_put(set, val1);
  REQUIRE_WITH_MSG(hset_len(set) == 1, "String set should have 1 element");
  REQUIRE_WITH_MSG(!hset_is_empty(set), "String set should not be empty");
  REQUIRE_WITH_MSG(hset_str_contains(set, val1),
                   "String set should contain added element");
  // Test adding duplicate (should not increase size)
  hset_str_put(set, val1);
  REQUIRE_WITH_MSG(hset_len(set) == 1,
                   "String set should still have 1 element after duplicate");
  REQUIRE_WITH_MSG(hset_str_contains(set, val1),
                   "String set should still contain element");
  // Test adding different elements
  char const* val2 = "world";
  char const* val3 = "test";
  char const* val4 = "example";
  hset_str_put(set, val2);
  hset_str_put(set, val3);
  hset_str_put(set, val4);
  REQUIRE_WITH_MSG(hset_len(set) == 4, "String set should have 4 unique elements");
  // Verify all elements are present
  REQUIRE_WITH_MSG(hset_str_contains(set, val1), "String set should contain 'hello'");
  REQUIRE_WITH_MSG(hset_str_contains(set, val2), "String set should contain 'world'");
  REQUIRE_WITH_MSG(hset_str_contains(set, val3), "String set should contain 'test'");
  REQUIRE_WITH_MSG(hset_str_contains(set, val4), "String set should contain 'example'");
  // Test non-existent elements
  char const* missing1 = "missing";
  char const* missing2 = "absent";
  REQUIRE_WITH_MSG(!hset_str_contains(set, missing1),
                   "String set should not contain 'missing'");
  REQUIRE_WITH_MSG(!hset_str_contains(set, missing2),
                   "String set should not contain 'absent'");
  hset_free(set);
}

void test_hset_str_remove_operations(void)
{
  char const** set = NULL;
  // Add some elements
  char const* words[]   = {"apple",
                           "banana",
                           "cherry",
                           "date",
                           "elderberry",
                           "fig",
                           "grape",
                           "honeydew",
                           "kiwi",
                           "lemon"};
  size_t      num_words = sizeof(words) / sizeof(words[0]);
  for (size_t i = 0; i < num_words; i++) {
    hset_str_put(set, words[i]);
  }
  REQUIRE_WITH_MSG(hset_len(set) == num_words, "String set should have all elements");
  // Remove some elements
  hset_str_remove(set, words[0]);  // "apple"
  hset_str_remove(set, words[4]);  // "elderberry"
  hset_str_remove(set, words[7]);  // "honeydew"
  REQUIRE_WITH_MSG(hset_len(set) == num_words - 3,
                   "String set should have 3 fewer elements");
  REQUIRE_WITH_MSG(!hset_str_contains(set, words[0]),
                   "Should not contain removed 'apple'");
  REQUIRE_WITH_MSG(!hset_str_contains(set, words[4]),
                   "Should not contain removed 'elderberry'");
  REQUIRE_WITH_MSG(!hset_str_contains(set, words[7]),
                   "Should not contain removed 'honeydew'");
  // Verify remaining elements are still there
  REQUIRE_WITH_MSG(hset_str_contains(set, words[1]), "Should still contain 'banana'");
  REQUIRE_WITH_MSG(hset_str_contains(set, words[9]), "Should still contain 'lemon'");
  // Remove non-existent element (should be safe)
  char const* missing = "nonexistent";
  hset_str_remove(set, missing);
  REQUIRE_WITH_MSG(hset_len(set) == num_words - 3,
                   "Length should not change after removing missing element");
  // Remove remaining elements
  for (size_t i = 1; i < num_words; i++) {
    if (i != 4 && i != 7) {  // Skip already removed elements
      hset_str_remove(set, words[i]);
    }
  }
  REQUIRE_WITH_MSG(hset_len(set) == 0, "String set should be empty after removing all");
  REQUIRE_WITH_MSG(hset_is_empty(set), "String set should report as empty");
  hset_free(set);
}

void test_hset_str_edge_cases(void)
{
  char const** set = NULL;
  // Test empty string
  char const* empty_str = "";
  hset_str_put(set, empty_str);
  REQUIRE_WITH_MSG(hset_len(set) == 1, "Should handle empty string");
  REQUIRE_WITH_MSG(hset_str_contains(set, empty_str), "Should contain empty string");
  // Test very long string
  char const* long_str =
    "this_is_a_very_long_string_that_tests_the_hash_function_behavior_with_"
    "extended_input_lengths_and_ensures_proper_collision_handling_mechanisms";
  hset_str_put(set, long_str);
  REQUIRE_WITH_MSG(hset_len(set) == 2, "Should handle very long string");
  REQUIRE_WITH_MSG(hset_str_contains(set, long_str), "Should contain long string");
  // Test strings with special characters
  char const* special1 = "hello\nworld";
  char const* special2 = "tab\tseparated";
  char const* special3 = "quote\"test";
  char const* special4 = "backslash\\test";
  hset_str_put(set, special1);
  hset_str_put(set, special2);
  hset_str_put(set, special3);
  hset_str_put(set, special4);
  REQUIRE_WITH_MSG(hset_len(set) == 6, "Should handle special characters");
  REQUIRE_WITH_MSG(hset_str_contains(set, special1), "Should contain newline string");
  REQUIRE_WITH_MSG(hset_str_contains(set, special2), "Should contain tab string");
  REQUIRE_WITH_MSG(hset_str_contains(set, special3), "Should contain quote string");
  REQUIRE_WITH_MSG(hset_str_contains(set, special4), "Should contain backslash string");
  // Test similar strings (potential hash collisions)
  char const* similar1 = "test001";
  char const* similar2 = "test002";
  char const* similar3 = "test003";
  char const* similar4 = "test004";
  hset_str_put(set, similar1);
  hset_str_put(set, similar2);
  hset_str_put(set, similar3);
  hset_str_put(set, similar4);
  REQUIRE_WITH_MSG(hset_len(set) == 10, "Should handle similar strings");
  REQUIRE_WITH_MSG(hset_str_contains(set, similar1), "Should contain 'test001'");
  REQUIRE_WITH_MSG(hset_str_contains(set, similar2), "Should contain 'test002'");
  REQUIRE_WITH_MSG(hset_str_contains(set, similar3), "Should contain 'test003'");
  REQUIRE_WITH_MSG(hset_str_contains(set, similar4), "Should contain 'test004'");
  hset_free(set);
}

void test_hset_str_large_scale(void)
{
  char const** set = NULL;
  // Test with many string elements
  char const* programming_languages[] = {
    "C",       "C++",    "Java",    "Python", "JavaScript",  "TypeScript", "Rust",
    "Go",      "Swift",  "Kotlin",  "Scala",  "Haskell",     "OCaml",      "F#",
    "Clojure", "Erlang", "Elixir",  "Ruby",   "PHP",         "Perl",       "Lua",
    "R",       "MATLAB", "Julia",   "Dart",   "Objective-C", "Assembly",   "COBOL",
    "Fortran", "Pascal", "Ada",     "Prolog", "Lisp",        "Scheme",     "Racket",
    "ML",      "Nim",    "Crystal", "Zig",    "D",           "V",          "Odin",
    "Carbon",  "Mojo",   "Gleam",   "Roc",    "Pony",        "Chapel",     "Fortress"};
  size_t num_languages = sizeof(programming_languages) / sizeof(programming_languages[0]);
  // Add all languages
  for (size_t i = 0; i < num_languages; i++) {
    hset_str_put(set, programming_languages[i]);
  }
  REQUIRE_WITH_MSG(hset_len(set) == num_languages,
                   "Should contain all programming languages");
  // Verify all languages are present
  for (size_t i = 0; i < num_languages; i++) {
    REQUIRE_WITH_MSG(hset_str_contains(set, programming_languages[i]),
                     "Should contain each programming language");
  }
  // Test some non-existent languages
  char const* fake_languages[] = {"C---", "Python++", "JavaRust", "GoScript"};
  for (size_t i = 0; i < 4; i++) {
    REQUIRE_WITH_MSG(!hset_str_contains(set, fake_languages[i]),
                     "Should not contain fake language");
  }
  // Remove every other language
  for (size_t i = 0; i < num_languages; i += 2) {
    hset_str_remove(set, programming_languages[i]);
  }
  size_t expected_remaining = num_languages - (num_languages + 1) / 2;
  REQUIRE_WITH_MSG(hset_len(set) == expected_remaining,
                   "Should have correct count after removals");
  // Verify odd-indexed languages remain
  for (size_t i = 1; i < num_languages; i += 2) {
    REQUIRE_WITH_MSG(hset_str_contains(set, programming_languages[i]),
                     "Odd-indexed languages should remain");
  }
  // Verify even-indexed languages are removed
  for (size_t i = 0; i < num_languages; i += 2) {
    REQUIRE_WITH_MSG(!hset_str_contains(set, programming_languages[i]),
                     "Even-indexed languages should be removed");
  }
  hset_free(set);
}

void test_hset_str_memory_operations(void)
{
  char const** set = NULL;
  // Test operations on NULL set
  REQUIRE_WITH_MSG(hset_len(set) == 0, "NULL set should have length 0");
  REQUIRE_WITH_MSG(hset_is_empty(set), "NULL set should be empty");
  REQUIRE_WITH_MSG(!hset_str_contains(set, "test"),
                   "NULL set should not contain anything");
  // Test reserve
  REQUIRE_WITH_MSG(hset_reserve(set, 20) == OK, "Reserve should succeed");
  REQUIRE_WITH_MSG(set != NULL, "Set should be allocated after reserve");
  // Fill with some elements
  char keys[15][10];
  for (int i = 0; i < 15; i++) {
    snprintf(keys[i], sizeof(keys[i]), "key_%d", i);
    hset_str_put(set, keys[i]);
  }
  REQUIRE_WITH_MSG(hset_len(set) == 15, "Should have 15 elements");
  // Remove all elements
  for (int i = 0; i < 15; i++) {
    hset_str_remove(set, keys[i]);
  }
  REQUIRE_WITH_MSG(hset_len(set) == 0, "Should be empty after removing all elements");
  REQUIRE_WITH_MSG(hset_is_empty(set), "Should report as empty");
  hset_free(set);
}

void test_hset_str_is_empty_comprehensive(void)
{
  char const** set = NULL;
  // Test empty states
  REQUIRE_WITH_MSG(hset_is_empty(set), "NULL set should be empty");
  // Test after reserve
  REQUIRE(OK == hset_reserve(set, 5));
  REQUIRE_WITH_MSG(hset_is_empty(set), "Reserved but unused set should be empty");
  // Test during population and clearing cycles
  for (int cycle = 0; cycle < 3; cycle++) {
    // Add elements
    for (int i = 0; i < 8; i++) {
      char key[20];
      snprintf(key, sizeof(key), "cycle_%d_item_%d", cycle, i);
      hset_str_put(set, key);
    }
    REQUIRE_WITH_MSG(!hset_is_empty(set), "Set should not be empty during population");
    // Remove elements
    for (int i = 0; i < 8; i++) {
      char key[20];
      snprintf(key, sizeof(key), "cycle_%d_item_%d", cycle, i);
      hset_str_remove(set, key);
    }
    REQUIRE_WITH_MSG(hset_is_empty(set), "Set should be empty after clearing");
  }
  hset_free(set);
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
  REQUIRE(deck[0] == 7);
  REQUIRE(deck_push(deck, ((size_t) {8}), 0) == OK);
  REQUIRE(deck_push(deck, ((size_t) {9}), 0) == OK);
  REQUIRE(deck_len(deck) == 3);
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
  deck_clear(deck);
  REQUIRE(deck_len(deck) == 0);
  REQUIRE(deck_max_depth(deck) == 0);
  // Re-use after clear.
  REQUIRE(deck_push(deck, ((size_t) {1}), 1) == OK);
  REQUIRE(deck_push(deck, ((size_t) {2}), 0) == OK);
  REQUIRE(deck_len(deck) == 2);
  REQUIRE(deck_max_depth(deck) == 1);
  REQUIRE(deck[0] == 1);
  REQUIRE(deck[1] == 2);
  deck_free(deck);
}

void test_deck_flatten(void)
{
  size_t* deck = _binary_deck(4);
  size_t  n    = deck_len(deck);
  REQUIRE(n == 16);
  deck_flatten(deck);
  REQUIRE(deck_max_depth(deck) == 1);
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
  deck_flatten(deck2);
  REQUIRE(deck_max_depth(deck2) == 0);
  REQUIRE(arr_len(_deck_header(deck2)->marks) == 0);
  deck_free(deck2);
}

void test_deck_reserve(void)
{
  size_t* deck = NULL;
  REQUIRE(deck_reserve(deck, 32) == OK);
  REQUIRE(deck != NULL);
  REQUIRE(deck_len(deck) == 0);
  for (size_t i = 0; i < 32; ++i) {
    REQUIRE(deck_push(deck, i, (i == 0) ? 1 : 0) == OK);
  }
  REQUIRE(deck_len(deck) == 32);
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
  REQUIRE(deck[0] == 42);
  REQUIRE(arr_len(_deck_header(deck)->marks) == 0);
  deck_free(deck);
  // Depth 1.
  size_t* deck2 = NULL;
  REQUIRE(deck_push(deck2, ((size_t) {7}), 1) == OK);
  REQUIRE(deck_len(deck2) == 1);
  REQUIRE(deck_max_depth(deck2) == 1);
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
  for (size_t i = 0; i < n; ++i) {
    REQUIRE(deck[i] == i);
  }
  deck_free(deck);
}

void test_deck_graft(void)
{
  // Graft a depth-3 binary deck: depth should increase by 1, items unchanged.
  size_t* deck = _binary_deck(3);
  deck_graft(deck);
  REQUIRE(deck_max_depth(deck) == 4);
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
  deck_flatten(deck2);
  REQUIRE(deck_max_depth(deck2) == 1);
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
    size_t*      deck    = _binary_deck(3);
    _DeckHeader* h       = _deck_header(deck);
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
