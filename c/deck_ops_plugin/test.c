#include <orc_sdk/orc_sdk.h>
#include <stdint.h>
#include <unity.h>

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

static void test_list_length_basic(void)
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

static void test_list_length_with_empty_lists(void)
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

static void test_list_length_single_list(void)
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

static void test_list_length_depth3(void)
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

static void test_list_length_wrong_n_inputs(void)
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

static void test_list_length_wrong_n_outputs(void)
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

static void test_list_length_null_input_ptr(void)
{
  /* Passing NULL as inputs pointer -> early return, no crash. */
  orc_sdk_init(NULL, NULL);
  OrcHandle out = {0};
  out.handle    = 2;
  LIST_LENGTH_INFO.func(0, NULL, 1, &out, 1);
  TEST_ASSERT_NULL(out.free_fn);
  TEST_ASSERT_NULL(out.items);
}

static void test_list_length_null_output_ptr(void)
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

static void test_list_length_output_free_fn_set(void)
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

static void test_list_length_output_id_preserved(void)
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

static void test_list_length_output_is_u64(void)
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

static void test_list_length_input_handle_unaffected(void)
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

static void test_list_length_reuse_output_same_type(void)
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

static void test_list_length_output_type_change(void)
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

static void test_list_length_clears_previous_output(void)
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

static void test_flatten_deck_basic(void)
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

static void test_flatten_deck_already_flat(void)
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

static void test_flatten_deck_multiple_inputs(void)
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

static void test_flatten_deck_integer_type(void)
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

static void test_flatten_deck_mismatched_counts(void)
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

static void test_flatten_deck_null_inputs_ptr(void)
{
  /* NULL inputs pointer -> early return, no crash. */
  orc_sdk_init(NULL, NULL);
  OrcHandle out = {0};
  out.handle    = 2;

  FLATTEN_DECK_INFO.func(0, NULL, 1, &out, 1);

  TEST_ASSERT_NULL(out.items);
  TEST_ASSERT_NULL(out.free_fn);
}

static void test_flatten_deck_null_outputs_ptr(void)
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

static void test_flatten_deck_no_create_proxy_capability(void)
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

static void test_flatten_deck_output_free_fn_set(void)
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

static void test_flatten_deck_output_type_matches_input(void)
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

static void test_flatten_deck_output_handle_preserved(void)
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

static void test_flatten_deck_dims_preserved(void)
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

/* ============================================================
   Serialization helper: write callback that appends to orc_sdk_arr
   ============================================================ */

typedef struct
{
  char **buf;
} _SerCtx;

static OrcError _test_write_fn(uint64_t const ctx, void const *data, uint64_t const len)
{
  _SerCtx *sc      = (_SerCtx *)(uintptr_t)ctx;
  size_t   cur_len = orc_sdk_arr_len(*sc->buf);
  orc_sdk_arr_resize(*sc->buf, cur_len + (size_t)len);
  memcpy(*sc->buf + cur_len, data, (size_t)len);
  return ORC_ERROR_NONE;
}

static void _init_sdk_with_serial_write(void)
{
  static OrcHost host         = {0};
  host.abi_version            = ORC_ABI_VERSION;
  host.callbacks.serial_write = _test_write_fn;
  orc_sdk_init(&host, NULL);
}

static char *_serialize_handle(OrcHandle const *h)
{
  char    *buf = NULL;
  _SerCtx  sc  = {&buf};
  OrcError err = orc_deck_serialize((uint64_t)(uintptr_t)&sc, h);
  TEST_ASSERT_EQUAL_UINT64(ORC_ERROR_NONE, err);
  return buf;
}

static OrcHandle _deserialize_handle(char const *buf, size_t len, uint64_t handle_id)
{
  OrcHandle out = {0};
  out.handle    = handle_id;
  OrcError err  = orc_deck_deserialize(0, buf, (uint64_t)len, &out);
  TEST_ASSERT_EQUAL_UINT64(ORC_ERROR_NONE, err);
  return out;
}

/* ============================================================
   orc_deck_serialize / orc_deck_deserialize
   ============================================================ */

static void test_serialize_round_trip_f64_flat(void)
{
  _init_sdk_with_serial_write();
  OrcHandle h = {0};
  h.handle    = 1;
  orc_sdk_handle_alloc(ORC_TYPE_F64, &h);
  ORC_SDK_DECK_INIT(h.items, double, (1.0, 2.0, 3.0));
  orc_sdk_oh_update(&h);
  char     *buf = _serialize_handle(&h);
  OrcHandle out = _deserialize_handle(buf, orc_sdk_arr_len(buf), 10);
  TEST_ASSERT_EQUAL_UINT64(ORC_TYPE_F64, out.type_id);
  TEST_ASSERT_EQUAL_UINT64(3, out.n_items);
  double const *items = (double const *)out.items;
  TEST_ASSERT_EQUAL_DOUBLE(1.0, items[0]);
  TEST_ASSERT_EQUAL_DOUBLE(2.0, items[1]);
  TEST_ASSERT_EQUAL_DOUBLE(3.0, items[2]);
  orc_sdk_handle_free(&h);
  orc_sdk_handle_free(&out);
  orc_sdk_arr_free(buf);
}

static void test_serialize_round_trip_i32_flat(void)
{
  _init_sdk_with_serial_write();
  OrcHandle h = {0};
  h.handle    = 1;
  orc_sdk_handle_alloc(ORC_TYPE_I32, &h);
  ORC_SDK_DECK_INIT(h.items, int32_t, (10, 20, 30, 40));
  orc_sdk_oh_update(&h);
  char     *buf = _serialize_handle(&h);
  OrcHandle out = _deserialize_handle(buf, orc_sdk_arr_len(buf), 10);
  TEST_ASSERT_EQUAL_UINT64(ORC_TYPE_I32, out.type_id);
  TEST_ASSERT_EQUAL_UINT64(4, out.n_items);
  int32_t const *items = (int32_t const *)out.items;
  TEST_ASSERT_EQUAL_INT32(10, items[0]);
  TEST_ASSERT_EQUAL_INT32(20, items[1]);
  TEST_ASSERT_EQUAL_INT32(30, items[2]);
  TEST_ASSERT_EQUAL_INT32(40, items[3]);
  orc_sdk_handle_free(&h);
  orc_sdk_handle_free(&out);
  orc_sdk_arr_free(buf);
}

static void test_serialize_round_trip_u8_flat(void)
{
  _init_sdk_with_serial_write();
  OrcHandle h = {0};
  h.handle    = 1;
  orc_sdk_handle_alloc(ORC_TYPE_U8, &h);
  ORC_SDK_DECK_INIT(h.items, uint8_t, (255, 0, 128));
  orc_sdk_oh_update(&h);
  char     *buf = _serialize_handle(&h);
  OrcHandle out = _deserialize_handle(buf, orc_sdk_arr_len(buf), 10);
  TEST_ASSERT_EQUAL_UINT64(ORC_TYPE_U8, out.type_id);
  TEST_ASSERT_EQUAL_UINT64(3, out.n_items);
  uint8_t const *items = (uint8_t const *)out.items;
  TEST_ASSERT_EQUAL_UINT8(255, items[0]);
  TEST_ASSERT_EQUAL_UINT8(0, items[1]);
  TEST_ASSERT_EQUAL_UINT8(128, items[2]);
  orc_sdk_handle_free(&h);
  orc_sdk_handle_free(&out);
  orc_sdk_arr_free(buf);
}

static void test_serialize_round_trip_f64_nested(void)
{
  _init_sdk_with_serial_write();
  OrcHandle h = {0};
  h.handle    = 1;
  orc_sdk_handle_alloc(ORC_TYPE_F64, &h);
  ORC_SDK_DECK_INIT(h.items, double, ((1.0, 2.0), (3.0)));
  orc_sdk_oh_update(&h);
  TEST_ASSERT_TRUE(h.n_marks > 0);
  char     *buf = _serialize_handle(&h);
  OrcHandle out = _deserialize_handle(buf, orc_sdk_arr_len(buf), 10);
  TEST_ASSERT_EQUAL_UINT64(ORC_TYPE_F64, out.type_id);
  TEST_ASSERT_EQUAL_UINT64(3, out.n_items);
  double const *items = (double const *)out.items;
  TEST_ASSERT_EQUAL_DOUBLE(1.0, items[0]);
  TEST_ASSERT_EQUAL_DOUBLE(2.0, items[1]);
  TEST_ASSERT_EQUAL_DOUBLE(3.0, items[2]);
  TEST_ASSERT_EQUAL_UINT64(h.n_marks, out.n_marks);
  for (uint64_t i = 0; i < h.n_marks; ++i) {
    TEST_ASSERT_EQUAL_UINT8(h.marks[i].depth, out.marks[i].depth);
    TEST_ASSERT_EQUAL_UINT64(h.marks[i].pos, out.marks[i].pos);
  }
  orc_sdk_handle_free(&h);
  orc_sdk_handle_free(&out);
  orc_sdk_arr_free(buf);
}

static void test_serialize_round_trip_empty_deck(void)
{
  _init_sdk_with_serial_write();
  OrcHandle h = {0};
  h.handle    = 1;
  orc_sdk_handle_alloc(ORC_TYPE_F32, &h);
  orc_sdk_oh_update(&h);
  TEST_ASSERT_EQUAL_UINT64(0, h.n_items);
  char     *buf = _serialize_handle(&h);
  OrcHandle out = _deserialize_handle(buf, orc_sdk_arr_len(buf), 10);
  TEST_ASSERT_EQUAL_UINT64(ORC_TYPE_F32, out.type_id);
  TEST_ASSERT_EQUAL_UINT64(0, out.n_items);
  orc_sdk_handle_free(&h);
  orc_sdk_handle_free(&out);
  orc_sdk_arr_free(buf);
}

static void test_serialize_preserves_dims(void)
{
  _init_sdk_with_serial_write();
  OrcHandle h = {0};
  h.handle    = 1;
  orc_sdk_handle_alloc(ORC_TYPE_F64, &h);
  ORC_SDK_DECK_INIT(h.items, double, (1.0, 2.0));
  h.dims[0] = 1;
  h.dims[1] = -2;
  h.dims[3] = 3;
  orc_sdk_oh_update(&h);
  char     *buf = _serialize_handle(&h);
  OrcHandle out = _deserialize_handle(buf, orc_sdk_arr_len(buf), 10);
  TEST_ASSERT_EQUAL_MEMORY(h.dims, out.dims, sizeof(OrcDims));
  orc_sdk_handle_free(&h);
  orc_sdk_handle_free(&out);
  orc_sdk_arr_free(buf);
}

static void test_deserialize_trailing_bytes_fails(void)
{
  _init_sdk_with_serial_write();
  OrcHandle h = {0};
  h.handle    = 1;
  orc_sdk_handle_alloc(ORC_TYPE_I64, &h);
  ORC_SDK_DECK_INIT(h.items, int64_t, (42));
  orc_sdk_oh_update(&h);
  char *buf = _serialize_handle(&h);
  orc_sdk_arr_push(buf, (char)0xFF);
  OrcHandle out = {0};
  out.handle    = 10;
  OrcError err  = orc_deck_deserialize(0, buf, orc_sdk_arr_len(buf), &out);
  TEST_ASSERT_TRUE(err != ORC_ERROR_NONE);
  orc_sdk_handle_free(&h);
  orc_sdk_arr_free(buf);
}

static void test_deserialize_truncated_fails(void)
{
  _init_sdk_with_serial_write();
  OrcHandle h = {0};
  h.handle    = 1;
  orc_sdk_handle_alloc(ORC_TYPE_F64, &h);
  ORC_SDK_DECK_INIT(h.items, double, (1.0, 2.0, 3.0));
  orc_sdk_oh_update(&h);
  char     *buf      = _serialize_handle(&h);
  size_t    full_len = orc_sdk_arr_len(buf);
  OrcHandle out      = {0};
  out.handle         = 10;
  OrcError err       = orc_deck_deserialize(0, buf, full_len - 1, &out);
  TEST_ASSERT_TRUE(err != ORC_ERROR_NONE);
  orc_sdk_handle_free(&h);
  orc_sdk_arr_free(buf);
}

static void test_deserialize_empty_buffer_fails(void)
{
  orc_sdk_init(NULL, NULL);
  OrcHandle out = {0};
  out.handle    = 10;
  OrcError err  = orc_deck_deserialize(0, NULL, 0, &out);
  TEST_ASSERT_TRUE(err != ORC_ERROR_NONE);
}

static void test_serialize_round_trip_all_primitive_types(void)
{
  _init_sdk_with_serial_write();
  /* u16 */
  {
    OrcHandle h = {.handle = 2};
    orc_sdk_handle_alloc(ORC_TYPE_U16, &h);
    ORC_SDK_DECK_INIT(h.items, uint16_t, (100, 200));
    orc_sdk_oh_update(&h);
    char     *buf = _serialize_handle(&h);
    OrcHandle out = _deserialize_handle(buf, orc_sdk_arr_len(buf), 101);
    TEST_ASSERT_EQUAL_UINT64(ORC_TYPE_U16, out.type_id);
    TEST_ASSERT_EQUAL_UINT16(100, ((uint16_t *)out.items)[0]);
    TEST_ASSERT_EQUAL_UINT16(200, ((uint16_t *)out.items)[1]);
    orc_sdk_handle_free(&h);
    orc_sdk_handle_free(&out);
    orc_sdk_arr_free(buf);
  }
  /* u32 */
  {
    OrcHandle h = {.handle = 3};
    orc_sdk_handle_alloc(ORC_TYPE_U32, &h);
    ORC_SDK_DECK_INIT(h.items, uint32_t, (1000, 2000));
    orc_sdk_oh_update(&h);
    char     *buf = _serialize_handle(&h);
    OrcHandle out = _deserialize_handle(buf, orc_sdk_arr_len(buf), 102);
    TEST_ASSERT_EQUAL_UINT64(ORC_TYPE_U32, out.type_id);
    TEST_ASSERT_EQUAL_UINT32(1000, ((uint32_t *)out.items)[0]);
    TEST_ASSERT_EQUAL_UINT32(2000, ((uint32_t *)out.items)[1]);
    orc_sdk_handle_free(&h);
    orc_sdk_handle_free(&out);
    orc_sdk_arr_free(buf);
  }
  /* u64 */
  {
    OrcHandle h = {.handle = 4};
    orc_sdk_handle_alloc(ORC_TYPE_U64, &h);
    ORC_SDK_DECK_INIT(h.items, uint64_t, (10000, 20000));
    orc_sdk_oh_update(&h);
    char     *buf = _serialize_handle(&h);
    OrcHandle out = _deserialize_handle(buf, orc_sdk_arr_len(buf), 103);
    TEST_ASSERT_EQUAL_UINT64(ORC_TYPE_U64, out.type_id);
    TEST_ASSERT_EQUAL_UINT64(10000, ((uint64_t *)out.items)[0]);
    TEST_ASSERT_EQUAL_UINT64(20000, ((uint64_t *)out.items)[1]);
    orc_sdk_handle_free(&h);
    orc_sdk_handle_free(&out);
    orc_sdk_arr_free(buf);
  }
  /* i8 */
  {
    OrcHandle h = {.handle = 5};
    orc_sdk_handle_alloc(ORC_TYPE_I8, &h);
    ORC_SDK_DECK_INIT(h.items, int8_t, (-1, 0, 1));
    orc_sdk_oh_update(&h);
    char     *buf = _serialize_handle(&h);
    OrcHandle out = _deserialize_handle(buf, orc_sdk_arr_len(buf), 104);
    TEST_ASSERT_EQUAL_UINT64(ORC_TYPE_I8, out.type_id);
    TEST_ASSERT_EQUAL_INT8(-1, ((int8_t *)out.items)[0]);
    TEST_ASSERT_EQUAL_INT8(0, ((int8_t *)out.items)[1]);
    TEST_ASSERT_EQUAL_INT8(1, ((int8_t *)out.items)[2]);
    orc_sdk_handle_free(&h);
    orc_sdk_handle_free(&out);
    orc_sdk_arr_free(buf);
  }
  /* i16 */
  {
    OrcHandle h = {.handle = 6};
    orc_sdk_handle_alloc(ORC_TYPE_I16, &h);
    ORC_SDK_DECK_INIT(h.items, int16_t, (-100, 0, 100));
    orc_sdk_oh_update(&h);
    char     *buf = _serialize_handle(&h);
    OrcHandle out = _deserialize_handle(buf, orc_sdk_arr_len(buf), 105);
    TEST_ASSERT_EQUAL_UINT64(ORC_TYPE_I16, out.type_id);
    TEST_ASSERT_EQUAL_INT16(-100, ((int16_t *)out.items)[0]);
    TEST_ASSERT_EQUAL_INT16(0, ((int16_t *)out.items)[1]);
    TEST_ASSERT_EQUAL_INT16(100, ((int16_t *)out.items)[2]);
    orc_sdk_handle_free(&h);
    orc_sdk_handle_free(&out);
    orc_sdk_arr_free(buf);
  }
  /* i64 */
  {
    OrcHandle h = {.handle = 7};
    orc_sdk_handle_alloc(ORC_TYPE_I64, &h);
    ORC_SDK_DECK_INIT(h.items, int64_t, (-10000, 0, 10000));
    orc_sdk_oh_update(&h);
    char     *buf = _serialize_handle(&h);
    OrcHandle out = _deserialize_handle(buf, orc_sdk_arr_len(buf), 106);
    TEST_ASSERT_EQUAL_UINT64(ORC_TYPE_I64, out.type_id);
    TEST_ASSERT_EQUAL_INT64(-10000, ((int64_t *)out.items)[0]);
    TEST_ASSERT_EQUAL_INT64(0, ((int64_t *)out.items)[1]);
    TEST_ASSERT_EQUAL_INT64(10000, ((int64_t *)out.items)[2]);
    orc_sdk_handle_free(&h);
    orc_sdk_handle_free(&out);
    orc_sdk_arr_free(buf);
  }
  /* f32 */
  {
    OrcHandle h = {.handle = 8};
    orc_sdk_handle_alloc(ORC_TYPE_F32, &h);
    ORC_SDK_DECK_INIT(h.items, float, (1.5f, -2.5f, 0.0f));
    orc_sdk_oh_update(&h);
    char     *buf = _serialize_handle(&h);
    OrcHandle out = _deserialize_handle(buf, orc_sdk_arr_len(buf), 107);
    TEST_ASSERT_EQUAL_UINT64(ORC_TYPE_F32, out.type_id);
    TEST_ASSERT_EQUAL_FLOAT(1.5f, ((float *)out.items)[0]);
    TEST_ASSERT_EQUAL_FLOAT(-2.5f, ((float *)out.items)[1]);
    TEST_ASSERT_EQUAL_FLOAT(0.0f, ((float *)out.items)[2]);
    orc_sdk_handle_free(&h);
    orc_sdk_handle_free(&out);
    orc_sdk_arr_free(buf);
  }
}

/* ============================================================
   orc_deck_alloc / orc_deck_free
   ============================================================ */

static void test_deck_alloc_f64(void)
{
  orc_sdk_init(NULL, NULL);
  OrcHandle h  = {0};
  h.handle     = 1;
  OrcError err = orc_deck_alloc(ORC_TYPE_F64, &h);
  TEST_ASSERT_EQUAL_UINT64(ORC_ERROR_NONE, err);
  TEST_ASSERT_EQUAL_UINT64(ORC_TYPE_F64, h.type_id);
  TEST_ASSERT_EQUAL_UINT64(sizeof(double), h.item_size);
  TEST_ASSERT_EQUAL_UINT64(0, h.n_items);
  TEST_ASSERT_NOT_NULL(h.free_fn);
  orc_sdk_handle_free(&h);
}

static void test_deck_alloc_i32(void)
{
  orc_sdk_init(NULL, NULL);
  OrcHandle h  = {0};
  h.handle     = 1;
  OrcError err = orc_deck_alloc(ORC_TYPE_I32, &h);
  TEST_ASSERT_EQUAL_UINT64(ORC_ERROR_NONE, err);
  TEST_ASSERT_EQUAL_UINT64(ORC_TYPE_I32, h.type_id);
  TEST_ASSERT_EQUAL_UINT64(sizeof(int32_t), h.item_size);
  orc_sdk_handle_free(&h);
}

static void test_deck_alloc_preserves_handle_id(void)
{
  orc_sdk_init(NULL, NULL);
  OrcHandle h = {0};
  h.handle    = 99;
  orc_deck_alloc(ORC_TYPE_F64, &h);
  TEST_ASSERT_EQUAL_UINT64(99, h.handle);
  orc_sdk_handle_free(&h);
}

static void test_deck_free_resets_handle(void)
{
  orc_sdk_init(NULL, NULL);
  OrcHandle h = {0};
  h.handle    = 1;
  orc_deck_alloc(ORC_TYPE_F32, &h);
  TEST_ASSERT_NOT_NULL(h.free_fn);
  orc_deck_free(&h);
  TEST_ASSERT_NULL(h.items);
  TEST_ASSERT_NULL(h.marks);
  TEST_ASSERT_EQUAL_UINT64(0, h.n_items);
}

static void test_deck_alloc_reuse_same_type(void)
{
  orc_sdk_init(NULL, NULL);
  OrcHandle h = {0};
  h.handle    = 1;
  orc_deck_alloc(ORC_TYPE_F64, &h);
  void const *ptr1 = h.items;
  orc_deck_alloc(ORC_TYPE_F64, &h);
  TEST_ASSERT_EQUAL_PTR(ptr1, h.items);
  TEST_ASSERT_EQUAL_UINT64(ORC_TYPE_F64, h.type_id);
  orc_sdk_handle_free(&h);
}

static void test_deck_alloc_type_change(void)
{
  orc_sdk_init(NULL, NULL);
  OrcHandle h = {0};
  h.handle    = 1;
  orc_deck_alloc(ORC_TYPE_F64, &h);
  TEST_ASSERT_EQUAL_UINT64(ORC_TYPE_F64, h.type_id);
  orc_deck_alloc(ORC_TYPE_I32, &h);
  TEST_ASSERT_EQUAL_UINT64(ORC_TYPE_I32, h.type_id);
  TEST_ASSERT_EQUAL_UINT64(sizeof(int32_t), h.item_size);
  orc_sdk_handle_free(&h);
}

/* Regression: deserialize must fail gracefully (not abort) when the serialized
   item_size doesn't match the type's actual size. */
static void test_deserialize_wrong_item_size_fails(void)
{
  _init_sdk_with_serial_write();
  OrcHandle h = {0};
  h.handle    = 1;
  orc_sdk_handle_alloc(ORC_TYPE_F64, &h);
  ORC_SDK_DECK_INIT(h.items, double, (1.0, 2.0));
  orc_sdk_oh_update(&h);
  char  *buf     = _serialize_handle(&h);
  size_t buf_len = orc_sdk_arr_len(buf);
  /* Corrupt the item_size field in the serialized buffer.
     Header layout: abi_version(8) + type_id(8) + dims(28) + n_items(8) + item_size(8)
     item_size starts at offset 52. Set it to 4 instead of 8. */
  uint64_t bad_size = 4;
  memcpy(buf + 52, &bad_size, sizeof(bad_size));
  OrcHandle out = {0};
  out.handle    = 10;
  OrcError err  = orc_deck_deserialize(0, buf, buf_len, &out);
  TEST_ASSERT_TRUE(err != ORC_ERROR_NONE);
  orc_sdk_handle_free(&h);
  orc_sdk_arr_free(buf);
}

/* Regression: deserialize must fail gracefully with an invalid type_id. */
static void test_deserialize_invalid_type_id_fails(void)
{
  _init_sdk_with_serial_write();
  OrcHandle h = {0};
  h.handle    = 1;
  orc_sdk_handle_alloc(ORC_TYPE_F64, &h);
  ORC_SDK_DECK_INIT(h.items, double, (1.0));
  orc_sdk_oh_update(&h);
  char  *buf     = _serialize_handle(&h);
  size_t buf_len = orc_sdk_arr_len(buf);
  /* Corrupt the type_id field. type_id starts at offset 8. */
  uint64_t bad_type = 0xFFFFFFFFFFFFFFFFull;
  memcpy(buf + 8, &bad_type, sizeof(bad_type));
  OrcHandle out = {0};
  out.handle    = 10;
  OrcError err  = orc_deck_deserialize(0, buf, buf_len, &out);
  TEST_ASSERT_TRUE(err != ORC_ERROR_NONE);
  orc_sdk_handle_free(&h);
  orc_sdk_arr_free(buf);
}

/* Regression: on deserialization error, the grown deck must be freed (not leaked)
   via handle_free. Verify that after a failed deserialize the handle is usable
   for a fresh alloc (i.e., no stale state). */
static void test_deserialize_error_cleans_up(void)
{
  _init_sdk_with_serial_write();
  OrcHandle h = {0};
  h.handle    = 1;
  orc_sdk_handle_alloc(ORC_TYPE_F64, &h);
  ORC_SDK_DECK_INIT(h.items, double, (1.0, 2.0, 3.0));
  orc_sdk_oh_update(&h);
  char  *buf     = _serialize_handle(&h);
  size_t buf_len = orc_sdk_arr_len(buf);
  /* Truncate to break deserialization after header is parsed (deck alloc
     will succeed but item read will fail). Keep enough for header + marks
     but cut off the item data. */
  size_t truncated = buf_len - 1;
  OrcHandle out    = {0};
  out.handle       = 10;
  OrcError err     = orc_deck_deserialize(0, buf, truncated, &out);
  TEST_ASSERT_TRUE(err != ORC_ERROR_NONE);
  /* The handle should be cleaned up — verify we can reuse the same handle id. */
  OrcHandle fresh = {0};
  fresh.handle    = 10;
  err = orc_sdk_handle_alloc(ORC_TYPE_F64, &fresh);
  TEST_ASSERT_EQUAL_UINT64(ORC_ERROR_NONE, err);
  orc_sdk_handle_free(&fresh);
  orc_sdk_handle_free(&h);
  orc_sdk_arr_free(buf);
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
  RUN_TEST(test_serialize_round_trip_f64_flat);
  RUN_TEST(test_serialize_round_trip_i32_flat);
  RUN_TEST(test_serialize_round_trip_u8_flat);
  RUN_TEST(test_serialize_round_trip_f64_nested);
  RUN_TEST(test_serialize_round_trip_empty_deck);
  RUN_TEST(test_serialize_preserves_dims);
  RUN_TEST(test_deserialize_trailing_bytes_fails);
  RUN_TEST(test_deserialize_truncated_fails);
  RUN_TEST(test_deserialize_empty_buffer_fails);
  RUN_TEST(test_serialize_round_trip_all_primitive_types);
  RUN_TEST(test_deck_alloc_f64);
  RUN_TEST(test_deck_alloc_i32);
  RUN_TEST(test_deck_alloc_preserves_handle_id);
  RUN_TEST(test_deck_free_resets_handle);
  RUN_TEST(test_deck_alloc_reuse_same_type);
  RUN_TEST(test_deck_alloc_type_change);
  RUN_TEST(test_deserialize_wrong_item_size_fails);
  RUN_TEST(test_deserialize_invalid_type_id_fails);
  RUN_TEST(test_deserialize_error_cleans_up);
  return UNITY_END();
}
