#include <orc_sdk/orc_sdk.h>
#include <stdint.h>
#include "unity.h"

extern OrcFuncInfo const LIST_LENGTH_INFO;

void setUp(void) {}
void tearDown(void) {}

/* Shorthand for the common 1-in / 1-out call. */
#define CALL_LIST_LENGTH(in_h, out_h) LIST_LENGTH_INFO.func(0, &(in_h), 1, &(out_h), 1)

/* ============================================================
   Correctness
   ============================================================ */

void test_list_length_basic(void)
{
  /* [[1,2,3],[4,5]] -> [3,2] */
  orc_sdk_init(NULL, NULL);
  OrcHandle in = {0}, out = {0};
  in.handle  = 1;
  out.handle = 2;
  orc_sdk_handle_alloc(ORC_TYPE_F64, &in);
  ORC_SDK_DECK_INIT(in.items, double, ((1.0, 2.0, 3.0), (4.0, 5.0)));
  orc_sdk_oh_update(&in);

  CALL_LIST_LENGTH(in, out);

  uint64_t const *result = (uint64_t const *)out.items;
  TEST_ASSERT_EQUAL_UINT64(2, out.n_items);
  TEST_ASSERT_EQUAL_UINT64(3, result[0]);
  TEST_ASSERT_EQUAL_UINT64(2, result[1]);
  orc_sdk_handle_free(&in);
  orc_sdk_handle_free(&out);
}

void test_list_length_with_empty_lists(void)
{
  /* [[],[1],[]] -> [0,1,0] */
  orc_sdk_init(NULL, NULL);
  OrcHandle in = {0}, out = {0};
  in.handle  = 1;
  out.handle = 2;
  orc_sdk_handle_alloc(ORC_TYPE_F64, &in);
  ORC_SDK_DECK_INIT(in.items, double, ((), (1.0), ()));
  orc_sdk_oh_update(&in);

  CALL_LIST_LENGTH(in, out);

  uint64_t const *result = (uint64_t const *)out.items;
  TEST_ASSERT_EQUAL_UINT64(3, out.n_items);
  TEST_ASSERT_EQUAL_UINT64(0, result[0]);
  TEST_ASSERT_EQUAL_UINT64(1, result[1]);
  TEST_ASSERT_EQUAL_UINT64(0, result[2]);
  orc_sdk_handle_free(&in);
  orc_sdk_handle_free(&out);
}

void test_list_length_single_list(void)
{
  /* [[42]] -> [1] */
  orc_sdk_init(NULL, NULL);
  OrcHandle in = {0}, out = {0};
  in.handle  = 1;
  out.handle = 2;
  orc_sdk_handle_alloc(ORC_TYPE_F64, &in);
  ORC_SDK_DECK_INIT(in.items, double, ((42.0)));
  orc_sdk_oh_update(&in);

  CALL_LIST_LENGTH(in, out);

  uint64_t const *result = (uint64_t const *)out.items;
  TEST_ASSERT_EQUAL_UINT64(1, out.n_items);
  TEST_ASSERT_EQUAL_UINT64(1, result[0]);
  orc_sdk_handle_free(&in);
  orc_sdk_handle_free(&out);
}

void test_list_length_depth3(void)
{
  /* Depth-3 input: (((1,2),(3)),((4,5,6)))
     Inner list lengths: group 0 -> [2,1], group 1 -> [3]
     Flat output items: [2,1,3] in a depth-2 deck. */
  orc_sdk_init(NULL, NULL);
  OrcHandle in = {0}, out = {0};
  in.handle  = 1;
  out.handle = 2;
  orc_sdk_handle_alloc(ORC_TYPE_F64, &in);
  ORC_SDK_DECK_INIT(in.items, double, (((1.0, 2.0), (3.0)), ((4.0, 5.0, 6.0))));
  orc_sdk_oh_update(&in);

  CALL_LIST_LENGTH(in, out);

  uint64_t const *result = (uint64_t const *)out.items;
  TEST_ASSERT_EQUAL_UINT64(3, out.n_items);
  TEST_ASSERT_EQUAL_UINT64(2, result[0]);
  TEST_ASSERT_EQUAL_UINT64(1, result[1]);
  TEST_ASSERT_EQUAL_UINT64(3, result[2]);
  orc_sdk_handle_free(&in);
  orc_sdk_handle_free(&out);
}

/* ============================================================
   Error / validation
   ============================================================ */

void test_list_length_wrong_n_inputs(void)
{
  /* n_inputs != 1 -> early return, output untouched. */
  orc_sdk_init(NULL, NULL);
  OrcHandle in = {0}, out = {0};
  in.handle  = 1;
  out.handle = 2;
  orc_sdk_handle_alloc(ORC_TYPE_F64, &in);
  ORC_SDK_DECK_INIT(in.items, double, ((1.0)));
  orc_sdk_oh_update(&in);

  LIST_LENGTH_INFO.func(0, &in, 2, &out, 1);

  TEST_ASSERT_NULL(out.free_fn);
  TEST_ASSERT_NULL(out.items);
  orc_sdk_handle_free(&in);
}

void test_list_length_wrong_n_outputs(void)
{
  /* n_outputs != 1 -> early return, output untouched. */
  orc_sdk_init(NULL, NULL);
  OrcHandle in = {0}, out = {0};
  in.handle  = 1;
  out.handle = 2;
  orc_sdk_handle_alloc(ORC_TYPE_F64, &in);
  ORC_SDK_DECK_INIT(in.items, double, ((1.0)));
  orc_sdk_oh_update(&in);

  LIST_LENGTH_INFO.func(0, &in, 1, &out, 2);

  TEST_ASSERT_NULL(out.free_fn);
  TEST_ASSERT_NULL(out.items);
  orc_sdk_handle_free(&in);
}

void test_list_length_null_input_ptr(void)
{
  /* Passing NULL as inputs pointer -> early return, no crash. */
  orc_sdk_init(NULL, NULL);
  OrcHandle out = {0};
  out.handle    = 2;

  LIST_LENGTH_INFO.func(0, NULL, 1, &out, 1);

  TEST_ASSERT_NULL(out.free_fn);
  TEST_ASSERT_NULL(out.items);
}

void test_list_length_null_output_ptr(void)
{
  /* Passing NULL as outputs pointer -> early return, no crash. */
  orc_sdk_init(NULL, NULL);
  OrcHandle in = {0};
  in.handle    = 1;
  orc_sdk_handle_alloc(ORC_TYPE_F64, &in);
  ORC_SDK_DECK_INIT(in.items, double, ((1.0)));
  orc_sdk_oh_update(&in);

  LIST_LENGTH_INFO.func(0, &in, 1, NULL, 1);

  orc_sdk_handle_free(&in);
}

/* ============================================================
   Ownership and lifetime invariants
   ============================================================ */

void test_list_length_output_free_fn_set(void)
{
  /* After a successful call, out.free_fn must be set (plugin owns the deck). */
  orc_sdk_init(NULL, NULL);
  OrcHandle in = {0}, out = {0};
  in.handle  = 1;
  out.handle = 2;
  orc_sdk_handle_alloc(ORC_TYPE_F64, &in);
  ORC_SDK_DECK_INIT(in.items, double, ((1.0)));
  orc_sdk_oh_update(&in);

  CALL_LIST_LENGTH(in, out);

  TEST_ASSERT_NOT_NULL(out.free_fn);
  orc_sdk_handle_free(&in);
  orc_sdk_handle_free(&out);
}

void test_list_length_output_id_preserved(void)
{
  /* out.handle must be unchanged before and after the call. */
  orc_sdk_init(NULL, NULL);
  OrcHandle in = {0}, out = {0};
  in.handle  = 1;
  out.handle = 99;
  orc_sdk_handle_alloc(ORC_TYPE_F64, &in);
  ORC_SDK_DECK_INIT(in.items, double, ((1.0)));
  orc_sdk_oh_update(&in);

  CALL_LIST_LENGTH(in, out);

  TEST_ASSERT_EQUAL_UINT64(99, out.handle);
  orc_sdk_handle_free(&in);
  orc_sdk_handle_free(&out);
}

void test_list_length_output_is_u64(void)
{
  /* list_length always allocates ORC_TYPE_U64 output regardless of input type. */
  orc_sdk_init(NULL, NULL);
  OrcHandle in = {0}, out = {0};
  in.handle  = 1;
  out.handle = 2;
  orc_sdk_handle_alloc(ORC_TYPE_F64, &in);
  ORC_SDK_DECK_INIT(in.items, double, ((1.0, 2.0)));
  orc_sdk_oh_update(&in);

  CALL_LIST_LENGTH(in, out);

  TEST_ASSERT_EQUAL_UINT64(ORC_TYPE_U64, out.type_id);
  orc_sdk_handle_free(&in);
  orc_sdk_handle_free(&out);
}

void test_list_length_input_handle_unaffected(void)
{
  /* The input handle's free_fn, items pointer, and n_items must not change. */
  orc_sdk_init(NULL, NULL);
  OrcHandle in = {0}, out = {0};
  in.handle  = 1;
  out.handle = 2;
  orc_sdk_handle_alloc(ORC_TYPE_F64, &in);
  ORC_SDK_DECK_INIT(in.items, double, ((1.0, 2.0), (3.0)));
  orc_sdk_oh_update(&in);

  void const *items_before                = in.items;
  uint64_t    n_items_before              = in.n_items;
  OrcError (*free_fn_before)(OrcHandle *) = in.free_fn;

  CALL_LIST_LENGTH(in, out);

  TEST_ASSERT_EQUAL_PTR(items_before, in.items);
  TEST_ASSERT_EQUAL_UINT64(n_items_before, in.n_items);
  TEST_ASSERT_EQUAL_PTR(free_fn_before, in.free_fn);
  orc_sdk_handle_free(&in);
  orc_sdk_handle_free(&out);
}

void test_list_length_reuse_output_same_type(void)
{
  /* Calling list_length twice on the same output handle rewrites it without appending. */
  orc_sdk_init(NULL, NULL);
  OrcHandle in = {0}, out = {0};
  in.handle  = 1;
  out.handle = 2;
  orc_sdk_handle_alloc(ORC_TYPE_F64, &in);

  /* First call: [[1,2,3],[4,5]] -> [3,2] */
  ORC_SDK_DECK_INIT(in.items, double, ((1.0, 2.0, 3.0), (4.0, 5.0)));
  orc_sdk_oh_update(&in);
  CALL_LIST_LENGTH(in, out);
  TEST_ASSERT_EQUAL_UINT64(2, out.n_items);
  void const *ptr_after_first = out.items;

  /* Second call: [[7]] -> [1] — fewer items, deck has capacity, no realloc. */
  ORC_SDK_DECK_INIT(in.items, double, ((7.0)));
  orc_sdk_oh_update(&in);
  CALL_LIST_LENGTH(in, out);

  uint64_t const *result = (uint64_t const *)out.items;
  TEST_ASSERT_EQUAL_UINT64(1, out.n_items);
  TEST_ASSERT_EQUAL_UINT64(1, result[0]);
  TEST_ASSERT_EQUAL_PTR(ptr_after_first, out.items);
  orc_sdk_handle_free(&in);
  orc_sdk_handle_free(&out);
}

void test_list_length_output_type_change(void)
{
  /* If out was previously allocated as a different type, list_length reallocates it as
   * u64. */
  orc_sdk_init(NULL, NULL);
  OrcHandle in = {0}, out = {0};
  in.handle  = 1;
  out.handle = 2;
  orc_sdk_handle_alloc(ORC_TYPE_F64, &in);
  /* Pre-allocate output as F64. */
  orc_sdk_handle_alloc(ORC_TYPE_F64, &out);
  TEST_ASSERT_EQUAL_UINT64(ORC_TYPE_F64, out.type_id);

  ORC_SDK_DECK_INIT(in.items, double, ((1.0, 2.0)));
  orc_sdk_oh_update(&in);
  CALL_LIST_LENGTH(in, out);

  TEST_ASSERT_EQUAL_UINT64(ORC_TYPE_U64, out.type_id);
  TEST_ASSERT_EQUAL_UINT64(2, out.handle);
  orc_sdk_handle_free(&in);
  orc_sdk_handle_free(&out);
}

void test_list_length_clears_previous_output(void)
{
  /* A second call with fewer output items must not leave stale data in the output. */
  orc_sdk_init(NULL, NULL);
  OrcHandle in = {0}, out = {0};
  in.handle  = 1;
  out.handle = 2;
  orc_sdk_handle_alloc(ORC_TYPE_F64, &in);

  /* First call: 3 singleton lists -> [1,1,1] */
  ORC_SDK_DECK_INIT(in.items, double, ((1.0), (2.0), (3.0)));
  orc_sdk_oh_update(&in);
  CALL_LIST_LENGTH(in, out);
  TEST_ASSERT_EQUAL_UINT64(3, out.n_items);
  void const *ptr_after_first = out.items;

  /* Second call: 1 list of 3 items -> [3] — fewer items, no realloc. */
  ORC_SDK_DECK_INIT(in.items, double, ((9.0, 8.0, 7.0)));
  orc_sdk_oh_update(&in);
  CALL_LIST_LENGTH(in, out);

  uint64_t const *result = (uint64_t const *)out.items;
  TEST_ASSERT_EQUAL_UINT64(1, out.n_items);
  TEST_ASSERT_EQUAL_UINT64(3, result[0]);
  TEST_ASSERT_EQUAL_PTR(ptr_after_first, out.items);
  orc_sdk_handle_free(&in);
  orc_sdk_handle_free(&out);
}

/* ============================================================ */

int main(void)
{
  UNITY_BEGIN();
  RUN_TEST(test_list_length_basic);
  RUN_TEST(test_list_length_with_empty_lists);
  RUN_TEST(test_list_length_single_list);
  RUN_TEST(test_list_length_depth3);
  RUN_TEST(test_list_length_wrong_n_inputs);
  RUN_TEST(test_list_length_wrong_n_outputs);
  RUN_TEST(test_list_length_null_input_ptr);
  RUN_TEST(test_list_length_null_output_ptr);
  RUN_TEST(test_list_length_output_free_fn_set);
  RUN_TEST(test_list_length_output_id_preserved);
  RUN_TEST(test_list_length_output_is_u64);
  RUN_TEST(test_list_length_input_handle_unaffected);
  RUN_TEST(test_list_length_reuse_output_same_type);
  RUN_TEST(test_list_length_output_type_change);
  RUN_TEST(test_list_length_clears_previous_output);
  return UNITY_END();
}
