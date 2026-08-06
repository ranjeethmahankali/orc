#include <orc_sdk/orc_sdk.h>
#include <stdint.h>
#include "unity.h"

extern OrcFuncInfo const LIST_LENGTH_INFO;
extern OrcFuncInfo const FLATTEN_DECK_INFO;

void _orc_sdk_registry_clear(void);  // Forward decl for a function defined in orc_sdk.c

void setUp(void)
{
  _orc_sdk_registry_clear();
}
void tearDown(void) {}

#define CALL_FLATTEN_DECK(in_h, out_h) FLATTEN_DECK_INFO.func(0, &(in_h), 1, &(out_h), 1)

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

/* ============================================================
   flatten_deck — Correctness
   ============================================================ */

void test_flatten_deck_basic(void)
{
  /* [[1,2,3],[4,5]] -> [1,2,3,4,5] */
  orc_sdk_init(NULL, NULL);
  OrcHandle in = {0}, out = {0};
  in.handle  = 1;
  out.handle = 2;
  orc_sdk_handle_alloc(ORC_TYPE_F64, &in);
  ORC_SDK_DECK_INIT(in.items, double, ((1.0, 2.0, 3.0), (4.0, 5.0)));
  orc_sdk_oh_update(&in);
  CALL_FLATTEN_DECK(in, out);
  double const *result = (double const *)out.items;
  TEST_ASSERT_EQUAL_UINT64(5, out.n_items);
  TEST_ASSERT_EQUAL_UINT64(1, out.n_marks);
  TEST_ASSERT_EQUAL_DOUBLE(1.0, result[0]);
  TEST_ASSERT_EQUAL_DOUBLE(2.0, result[1]);
  TEST_ASSERT_EQUAL_DOUBLE(3.0, result[2]);
  TEST_ASSERT_EQUAL_DOUBLE(4.0, result[3]);
  TEST_ASSERT_EQUAL_DOUBLE(5.0, result[4]);
  orc_sdk_handle_free(&in);
  orc_sdk_handle_free(&out);
}

void test_flatten_deck_already_flat(void)
{
  /* A depth-1 (already flat) input comes out unchanged. */
  orc_sdk_init(NULL, NULL);
  OrcHandle in = {0}, out = {0};
  in.handle  = 1;
  out.handle = 2;
  orc_sdk_handle_alloc(ORC_TYPE_F64, &in);
  ORC_SDK_DECK_INIT(in.items, double, (1.0, 2.0, 3.0));
  orc_sdk_oh_update(&in);
  CALL_FLATTEN_DECK(in, out);
  double const *result = (double const *)out.items;
  TEST_ASSERT_EQUAL_UINT64(3, out.n_items);
  TEST_ASSERT_EQUAL_UINT64(1, out.n_marks);
  TEST_ASSERT_EQUAL_DOUBLE(1.0, result[0]);
  TEST_ASSERT_EQUAL_DOUBLE(2.0, result[1]);
  TEST_ASSERT_EQUAL_DOUBLE(3.0, result[2]);
  orc_sdk_handle_free(&in);
  orc_sdk_handle_free(&out);
}

void test_flatten_deck_multiple_inputs(void)
{
  /* Two inputs flattened independently: [[1,2],[3]] and [[4,5,6]] -> [1,2,3] and [4,5,6].
   * This verifies the loop correctly advances through the input/output arrays. */
  orc_sdk_init(NULL, NULL);
  OrcHandle ins[2]  = {{0}, {0}};
  OrcHandle outs[2] = {{0}, {0}};
  ins[0].handle     = 1;
  ins[1].handle     = 2;
  outs[0].handle    = 3;
  outs[1].handle    = 4;
  orc_sdk_handle_alloc(ORC_TYPE_F64, &ins[0]);
  ORC_SDK_DECK_INIT(ins[0].items, double, ((1.0, 2.0), (3.0)));
  orc_sdk_oh_update(&ins[0]);
  orc_sdk_handle_alloc(ORC_TYPE_F64, &ins[1]);
  ORC_SDK_DECK_INIT(ins[1].items, double, ((4.0, 5.0, 6.0)));
  orc_sdk_oh_update(&ins[1]);
  FLATTEN_DECK_INFO.func(0, ins, 2, outs, 2);
  double const *r0 = (double const *)outs[0].items;
  double const *r1 = (double const *)outs[1].items;
  TEST_ASSERT_EQUAL_UINT64(3, outs[0].n_items);
  TEST_ASSERT_EQUAL_UINT64(1, outs[0].n_marks);
  TEST_ASSERT_EQUAL_DOUBLE(1.0, r0[0]);
  TEST_ASSERT_EQUAL_DOUBLE(2.0, r0[1]);
  TEST_ASSERT_EQUAL_DOUBLE(3.0, r0[2]);
  TEST_ASSERT_EQUAL_UINT64(3, outs[1].n_items);
  TEST_ASSERT_EQUAL_UINT64(1, outs[1].n_marks);
  TEST_ASSERT_EQUAL_DOUBLE(4.0, r1[0]);
  TEST_ASSERT_EQUAL_DOUBLE(5.0, r1[1]);
  TEST_ASSERT_EQUAL_DOUBLE(6.0, r1[2]);
  orc_sdk_handle_free(&ins[0]);
  orc_sdk_handle_free(&ins[1]);
  orc_sdk_handle_free(&outs[0]);
  orc_sdk_handle_free(&outs[1]);
}

void test_flatten_deck_integer_type(void)
{
  /* Works for u32, not just f64. [[10,20],[30]] -> [10,20,30] */
  orc_sdk_init(NULL, NULL);
  OrcHandle in = {0}, out = {0};
  in.handle  = 1;
  out.handle = 2;
  orc_sdk_handle_alloc(ORC_TYPE_U32, &in);
  ORC_SDK_DECK_INIT(in.items, uint32_t, ((10u, 20u), (30u)));
  orc_sdk_oh_update(&in);
  CALL_FLATTEN_DECK(in, out);
  uint32_t const *result = (uint32_t const *)out.items;
  TEST_ASSERT_EQUAL_UINT64(ORC_TYPE_U32, out.type_id);
  TEST_ASSERT_EQUAL_UINT64(3, out.n_items);
  TEST_ASSERT_EQUAL_UINT32(10u, result[0]);
  TEST_ASSERT_EQUAL_UINT32(20u, result[1]);
  TEST_ASSERT_EQUAL_UINT32(30u, result[2]);
  orc_sdk_handle_free(&in);
  orc_sdk_handle_free(&out);
}

/* ============================================================
   flatten_deck — Error / validation
   ============================================================ */

void test_flatten_deck_mismatched_counts(void)
{
  /* n_inputs != n_outputs -> early return, output untouched. */
  orc_sdk_init(NULL, NULL);
  OrcHandle in = {0}, out = {0};
  in.handle  = 1;
  out.handle = 2;
  orc_sdk_handle_alloc(ORC_TYPE_F64, &in);
  ORC_SDK_DECK_INIT(in.items, double, ((1.0)));
  orc_sdk_oh_update(&in);

  FLATTEN_DECK_INFO.func(0, &in, 2, &out, 1);

  TEST_ASSERT_NULL(out.items);
  TEST_ASSERT_NULL(out.free_fn);
  orc_sdk_handle_free(&in);
}

void test_flatten_deck_null_inputs_ptr(void)
{
  /* NULL inputs pointer -> early return, no crash. */
  orc_sdk_init(NULL, NULL);
  OrcHandle out = {0};
  out.handle    = 2;

  FLATTEN_DECK_INFO.func(0, NULL, 1, &out, 1);

  TEST_ASSERT_NULL(out.items);
  TEST_ASSERT_NULL(out.free_fn);
}

void test_flatten_deck_null_outputs_ptr(void)
{
  /* NULL outputs pointer -> early return, no crash. */
  orc_sdk_init(NULL, NULL);
  OrcHandle in = {0};
  in.handle    = 1;
  orc_sdk_handle_alloc(ORC_TYPE_F64, &in);
  ORC_SDK_DECK_INIT(in.items, double, ((1.0)));
  orc_sdk_oh_update(&in);

  FLATTEN_DECK_INFO.func(0, &in, 1, NULL, 1);

  orc_sdk_handle_free(&in);
}

void test_flatten_deck_no_create_proxy_capability(void)
{
  /* An input with an unknown type triggers the host fallback path.
   * orc_sdk_init(NULL, NULL) leaves HOST.create_deck_from_proxy == NULL, so
   * orc_sdk_host_create_proxy_deck returns ORC_ERROR_MISSING_CAPABILITY.
   * The output must remain uninitialized. */
  orc_sdk_init(NULL, NULL);
  OrcHandle in = {0}, out = {0};
  in.handle  = 1;
  out.handle = 2;
  in.type_id = 0x99u; /* unknown type — not handled by is_type_known */

  FLATTEN_DECK_INFO.func(0, &in, 1, &out, 1);

  TEST_ASSERT_EQUAL_UINT64(2, out.handle);
  TEST_ASSERT_NULL(out.items);
  TEST_ASSERT_NULL(out.free_fn);
}

/* ============================================================
   flatten_deck — Ownership and lifetime invariants
   ============================================================ */

void test_flatten_deck_output_free_fn_set(void)
{
  /* After a successful call, out.free_fn must be set (plugin owns the deck). */
  orc_sdk_init(NULL, NULL);
  OrcHandle in = {0}, out = {0};
  in.handle  = 1;
  out.handle = 2;
  orc_sdk_handle_alloc(ORC_TYPE_F64, &in);
  ORC_SDK_DECK_INIT(in.items, double, ((1.0, 2.0)));
  orc_sdk_oh_update(&in);

  CALL_FLATTEN_DECK(in, out);

  TEST_ASSERT_NOT_NULL(out.free_fn);
  orc_sdk_handle_free(&in);
  orc_sdk_handle_free(&out);
}

void test_flatten_deck_output_type_matches_input(void)
{
  /* The output type_id must match the input type_id. */
  orc_sdk_init(NULL, NULL);
  OrcHandle in = {0}, out = {0};
  in.handle  = 1;
  out.handle = 2;
  orc_sdk_handle_alloc(ORC_TYPE_F32, &in);
  ORC_SDK_DECK_INIT(in.items, float, ((1.0f, 2.0f)));
  orc_sdk_oh_update(&in);

  CALL_FLATTEN_DECK(in, out);

  TEST_ASSERT_EQUAL_UINT64(ORC_TYPE_F32, out.type_id);
  orc_sdk_handle_free(&in);
  orc_sdk_handle_free(&out);
}

void test_flatten_deck_output_handle_preserved(void)
{
  /* out.handle must be unchanged after the call. */
  orc_sdk_init(NULL, NULL);
  OrcHandle in = {0}, out = {0};
  in.handle  = 1;
  out.handle = 99;
  orc_sdk_handle_alloc(ORC_TYPE_F64, &in);
  ORC_SDK_DECK_INIT(in.items, double, ((1.0)));
  orc_sdk_oh_update(&in);

  CALL_FLATTEN_DECK(in, out);

  TEST_ASSERT_EQUAL_UINT64(99, out.handle);
  orc_sdk_handle_free(&in);
  orc_sdk_handle_free(&out);
}

void test_flatten_deck_dims_preserved(void)
{
  /* Dims from the input must be propagated to the output. */
  orc_sdk_init(NULL, NULL);
  OrcHandle in = {0}, out = {0};
  in.handle  = 1;
  out.handle = 2;
  orc_sdk_handle_alloc(ORC_TYPE_F64, &in);
  /* Set dims after alloc — alloc zeroes them. */
  in.dims[ORC_DIM_LENGTH] = 1;
  in.dims[ORC_DIM_TIME]   = -2;
  ORC_SDK_DECK_INIT(in.items, double, ((1.0, 2.0)));
  orc_sdk_oh_update(&in);

  CALL_FLATTEN_DECK(in, out);

  TEST_ASSERT_EQUAL_INT32(1, out.dims[ORC_DIM_LENGTH]);
  TEST_ASSERT_EQUAL_INT32(-2, out.dims[ORC_DIM_TIME]);
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
  RUN_TEST(test_flatten_deck_basic);
  RUN_TEST(test_flatten_deck_already_flat);
  RUN_TEST(test_flatten_deck_multiple_inputs);
  RUN_TEST(test_flatten_deck_integer_type);
  RUN_TEST(test_flatten_deck_mismatched_counts);
  RUN_TEST(test_flatten_deck_null_inputs_ptr);
  RUN_TEST(test_flatten_deck_null_outputs_ptr);
  RUN_TEST(test_flatten_deck_no_create_proxy_capability);
  RUN_TEST(test_flatten_deck_output_free_fn_set);
  RUN_TEST(test_flatten_deck_output_type_matches_input);
  RUN_TEST(test_flatten_deck_output_handle_preserved);
  RUN_TEST(test_flatten_deck_dims_preserved);
  return UNITY_END();
}
