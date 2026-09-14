#include <orc_sdk/orc_sdk.h>
#include <stdint.h>
#include <unity.h>

extern OrcFuncInfo const MAKE_VEC_INFO;
extern OrcFuncInfo const VEC_ADD_INFO;
extern OrcFuncInfo const VEC_SUBTRACT_INFO;
extern OrcFuncInfo const VEC_DOT_PRODUCT_INFO;

void _orc_sdk_registry_clear(void);  // Forward decl for a function defined in orc_sdk.c

void setUp(void)
{
  _orc_sdk_registry_clear();
}
void tearDown(void) {}

typedef struct
{
  double x, y;
} _Vec2;

typedef struct
{
  double x, y, z;
} _Vec3;

typedef struct
{
  float x, y, z;
} _FVec3;

/* ============================================================
   make_vec — Correctness
   ============================================================ */

static void test_make_vec_two_scalars_f64(void)
{
  /* (1.0) + (2.0) -> (1.0, 2.0), a dvec2. */
  orc_sdk_init(NULL, NULL);
  OrcHandle in[2] = {{0}, {0}}, out = {0};
  in[0].handle = 1;
  in[1].handle = 2;
  out.handle   = 3;
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(double), &in[0]);
  ORC_SDK_DECK_INIT(in[0].items, double, (1.0));
  orc_sdk_oh_update(&in[0]);
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(double), &in[1]);
  ORC_SDK_DECK_INIT(in[1].items, double, (2.0));
  orc_sdk_oh_update(&in[1]);

  OrcError err = MAKE_VEC_INFO.func(0, in, 2, &out, 1);

  TEST_ASSERT_EQUAL_UINT64(ORC_ERROR_NONE, err);
  TEST_ASSERT_EQUAL_UINT64(ORC_TYPE_F64, out.type_id);
  TEST_ASSERT_EQUAL_UINT64(2 * sizeof(double), out.item_size);
  TEST_ASSERT_EQUAL_UINT64(1, out.n_items);
  double const *result = (double const *)out.items;
  TEST_ASSERT_EQUAL_DOUBLE(1.0, result[0]);
  TEST_ASSERT_EQUAL_DOUBLE(2.0, result[1]);
  orc_sdk_handle_free(&in[0]);
  orc_sdk_handle_free(&in[1]);
  orc_sdk_handle_free(&out);
}

static void test_make_vec_three_scalars_f64(void)
{
  /* (1.0) + (2.0) + (3.0) -> (1.0, 2.0, 3.0), a dvec3. */
  orc_sdk_init(NULL, NULL);
  OrcHandle in[3] = {{0}, {0}, {0}}, out = {0};
  in[0].handle = 1;
  in[1].handle = 2;
  in[2].handle = 3;
  out.handle   = 4;
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(double), &in[0]);
  ORC_SDK_DECK_INIT(in[0].items, double, (1.0));
  orc_sdk_oh_update(&in[0]);
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(double), &in[1]);
  ORC_SDK_DECK_INIT(in[1].items, double, (2.0));
  orc_sdk_oh_update(&in[1]);
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(double), &in[2]);
  ORC_SDK_DECK_INIT(in[2].items, double, (3.0));
  orc_sdk_oh_update(&in[2]);

  OrcError err = MAKE_VEC_INFO.func(0, in, 3, &out, 1);

  TEST_ASSERT_EQUAL_UINT64(ORC_ERROR_NONE, err);
  TEST_ASSERT_EQUAL_UINT64(3 * sizeof(double), out.item_size);
  TEST_ASSERT_EQUAL_UINT64(1, out.n_items);
  double const *result = (double const *)out.items;
  TEST_ASSERT_EQUAL_DOUBLE(1.0, result[0]);
  TEST_ASSERT_EQUAL_DOUBLE(2.0, result[1]);
  TEST_ASSERT_EQUAL_DOUBLE(3.0, result[2]);
  orc_sdk_handle_free(&in[0]);
  orc_sdk_handle_free(&in[1]);
  orc_sdk_handle_free(&in[2]);
  orc_sdk_handle_free(&out);
}

static void test_make_vec_mixed_arity_components(void)
{
  /* A dvec2 (arity 2) combined with a scalar (arity 1) -> a dvec3.
     Regression test: the output arity must be the *sum* of the component
     arities (2 + 1 = 3), not n_inputs (2) and not the first component's
     arity repeated for every input. */
  orc_sdk_init(NULL, NULL);
  OrcHandle in[2] = {{0}, {0}}, out = {0};
  in[0].handle = 1;
  in[1].handle = 2;
  out.handle   = 3;
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(_Vec2), &in[0]);
  _Vec2 *vdeck = (_Vec2 *)in[0].items;
  orc_sdk_deck_push(vdeck, ((_Vec2) {10.0, 20.0}), 0);
  in[0].items = vdeck;
  orc_sdk_oh_update(&in[0]);
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(double), &in[1]);
  ORC_SDK_DECK_INIT(in[1].items, double, (30.0));
  orc_sdk_oh_update(&in[1]);

  OrcError err = MAKE_VEC_INFO.func(0, in, 2, &out, 1);

  TEST_ASSERT_EQUAL_UINT64(ORC_ERROR_NONE, err);
  TEST_ASSERT_EQUAL_UINT64(3 * sizeof(double), out.item_size);
  TEST_ASSERT_EQUAL_UINT64(1, out.n_items);
  double const *result = (double const *)out.items;
  TEST_ASSERT_EQUAL_DOUBLE(10.0, result[0]);
  TEST_ASSERT_EQUAL_DOUBLE(20.0, result[1]);
  TEST_ASSERT_EQUAL_DOUBLE(30.0, result[2]);
  orc_sdk_handle_free(&in[0]);
  orc_sdk_handle_free(&in[1]);
  orc_sdk_handle_free(&out);
}

static void test_make_vec_multi_row_broadcast(void)
{
  /* Two flat, 3-item scalar decks -> 3 dvec2 rows, one per matching pair.
     (1,10) (2,20) (3,30) -> (1,10),(2,20),(3,30). */
  orc_sdk_init(NULL, NULL);
  OrcHandle in[2] = {{0}, {0}}, out = {0};
  in[0].handle = 1;
  in[1].handle = 2;
  out.handle   = 3;
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(double), &in[0]);
  ORC_SDK_DECK_INIT(in[0].items, double, (1.0, 2.0, 3.0));
  orc_sdk_oh_update(&in[0]);
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(double), &in[1]);
  ORC_SDK_DECK_INIT(in[1].items, double, (10.0, 20.0, 30.0));
  orc_sdk_oh_update(&in[1]);

  OrcError err = MAKE_VEC_INFO.func(0, in, 2, &out, 1);

  TEST_ASSERT_EQUAL_UINT64(ORC_ERROR_NONE, err);
  TEST_ASSERT_EQUAL_UINT64(3, out.n_items);
  TEST_ASSERT_EQUAL_UINT64(2 * sizeof(double), out.item_size);
  double const *result = (double const *)out.items;
  TEST_ASSERT_EQUAL_DOUBLE(1.0, result[0]);
  TEST_ASSERT_EQUAL_DOUBLE(10.0, result[1]);
  TEST_ASSERT_EQUAL_DOUBLE(2.0, result[2]);
  TEST_ASSERT_EQUAL_DOUBLE(20.0, result[3]);
  TEST_ASSERT_EQUAL_DOUBLE(3.0, result[4]);
  TEST_ASSERT_EQUAL_DOUBLE(30.0, result[5]);
  orc_sdk_handle_free(&in[0]);
  orc_sdk_handle_free(&in[1]);
  orc_sdk_handle_free(&out);
}

static void test_make_vec_integer_type(void)
{
  /* Works for i32, not just f64. (7) + (-3) -> (7, -3). */
  orc_sdk_init(NULL, NULL);
  OrcHandle in[2] = {{0}, {0}}, out = {0};
  in[0].handle = 1;
  in[1].handle = 2;
  out.handle   = 3;
  orc_sdk_handle_alloc(ORC_TYPE_I32, sizeof(int32_t), &in[0]);
  ORC_SDK_DECK_INIT(in[0].items, int32_t, (7));
  orc_sdk_oh_update(&in[0]);
  orc_sdk_handle_alloc(ORC_TYPE_I32, sizeof(int32_t), &in[1]);
  ORC_SDK_DECK_INIT(in[1].items, int32_t, (-3));
  orc_sdk_oh_update(&in[1]);

  OrcError err = MAKE_VEC_INFO.func(0, in, 2, &out, 1);

  TEST_ASSERT_EQUAL_UINT64(ORC_ERROR_NONE, err);
  TEST_ASSERT_EQUAL_UINT64(ORC_TYPE_I32, out.type_id);
  TEST_ASSERT_EQUAL_UINT64(2 * sizeof(int32_t), out.item_size);
  int32_t const *result = (int32_t const *)out.items;
  TEST_ASSERT_EQUAL_INT32(7, result[0]);
  TEST_ASSERT_EQUAL_INT32(-3, result[1]);
  orc_sdk_handle_free(&in[0]);
  orc_sdk_handle_free(&in[1]);
  orc_sdk_handle_free(&out);
}

/* ============================================================
   make_vec — Error / validation
   ============================================================ */

static void test_make_vec_too_few_inputs(void)
{
  /* n_inputs < 2 -> early return, output untouched. */
  orc_sdk_init(NULL, NULL);
  OrcHandle in = {0}, out = {0};
  in.handle  = 1;
  out.handle = 2;
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(double), &in);
  ORC_SDK_DECK_INIT(in.items, double, (1.0));
  orc_sdk_oh_update(&in);

  OrcError err = MAKE_VEC_INFO.func(0, &in, 1, &out, 1);

  TEST_ASSERT_EQUAL_UINT64(ORC_ERROR_INVALID_ARGUMENTS, err);
  TEST_ASSERT_NULL(out.items);
  TEST_ASSERT_NULL(out.free_fn);
  orc_sdk_handle_free(&in);
}

static void test_make_vec_wrong_n_outputs(void)
{
  /* n_outputs != 1 -> early return, output untouched. */
  orc_sdk_init(NULL, NULL);
  OrcHandle in[2] = {{0}, {0}}, out = {0};
  in[0].handle = 1;
  in[1].handle = 2;
  out.handle   = 3;
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(double), &in[0]);
  ORC_SDK_DECK_INIT(in[0].items, double, (1.0));
  orc_sdk_oh_update(&in[0]);
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(double), &in[1]);
  ORC_SDK_DECK_INIT(in[1].items, double, (2.0));
  orc_sdk_oh_update(&in[1]);

  OrcError err = MAKE_VEC_INFO.func(0, in, 2, &out, 2);

  TEST_ASSERT_EQUAL_UINT64(ORC_ERROR_INVALID_ARGUMENTS, err);
  TEST_ASSERT_NULL(out.items);
  TEST_ASSERT_NULL(out.free_fn);
  orc_sdk_handle_free(&in[0]);
  orc_sdk_handle_free(&in[1]);
}

static void test_make_vec_rejects_non_primitive_type(void)
{
  /* Vector components must be primitive scalar types -- proxies are rejected. */
  orc_sdk_init(NULL, NULL);
  OrcHandle in[2] = {{0}, {0}}, out = {0};
  in[0].handle = 1;
  in[1].handle = 2;
  out.handle   = 3;
  orc_sdk_handle_alloc(ORC_TYPE_PROXY, sizeof(OrcItemProxy), &in[0]);
  orc_sdk_handle_alloc(ORC_TYPE_PROXY, sizeof(OrcItemProxy), &in[1]);

  OrcError err = MAKE_VEC_INFO.func(0, in, 2, &out, 1);

  TEST_ASSERT_EQUAL_UINT64(ORC_ERROR_TYPE_MISMATCH, err);
  TEST_ASSERT_NULL(out.items);
  TEST_ASSERT_NULL(out.free_fn);
  orc_sdk_handle_free(&in[0]);
  orc_sdk_handle_free(&in[1]);
}

static void test_make_vec_rejects_mismatched_component_types(void)
{
  /* All components must be of the same type -- f64 mixed with i32 is rejected. */
  orc_sdk_init(NULL, NULL);
  OrcHandle in[2] = {{0}, {0}}, out = {0};
  in[0].handle = 1;
  in[1].handle = 2;
  out.handle   = 3;
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(double), &in[0]);
  ORC_SDK_DECK_INIT(in[0].items, double, (1.0));
  orc_sdk_oh_update(&in[0]);
  orc_sdk_handle_alloc(ORC_TYPE_I32, sizeof(int32_t), &in[1]);
  ORC_SDK_DECK_INIT(in[1].items, int32_t, (2));
  orc_sdk_oh_update(&in[1]);

  OrcError err = MAKE_VEC_INFO.func(0, in, 2, &out, 1);

  TEST_ASSERT_EQUAL_UINT64(ORC_ERROR_INVALID_ARGUMENTS, err);
  TEST_ASSERT_NULL(out.items);
  TEST_ASSERT_NULL(out.free_fn);
  orc_sdk_handle_free(&in[0]);
  orc_sdk_handle_free(&in[1]);
}

static void test_make_vec_rejects_zero_item_size(void)
{
  /* A component with item_size == 0 is an invalid handle. */
  orc_sdk_init(NULL, NULL);
  OrcHandle in[2] = {{0}, {0}}, out = {0};
  in[0].handle = 1;
  in[1].handle = 2;
  out.handle   = 3;
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(double), &in[0]);
  ORC_SDK_DECK_INIT(in[0].items, double, (1.0));
  orc_sdk_oh_update(&in[0]);
  in[1].type_id = ORC_TYPE_F64; /* item_size left at 0 -- never allocated. */

  OrcError err = MAKE_VEC_INFO.func(0, in, 2, &out, 1);

  TEST_ASSERT_EQUAL_UINT64(ORC_ERROR_INVALID_HANDLE, err);
  TEST_ASSERT_NULL(out.items);
  TEST_ASSERT_NULL(out.free_fn);
  orc_sdk_handle_free(&in[0]);
}

static void test_make_vec_rejects_invalid_aggregate_item_size(void)
{
  /* A component's item_size must be a whole multiple of the scalar size.
     sizeof(double) == 8, so item_size == 12 is invalid. Constructed via direct field
     assignment: orc_sdk_handle_alloc itself already rejects item_size % scalar_size != 0,
     so it can't be used to create this handle -- we're testing make_vec's own defensive
     check against a handle that could have been produced some other way. */
  orc_sdk_init(NULL, NULL);
  OrcHandle in[2] = {{0}, {0}}, out = {0};
  in[0].handle = 1;
  in[1].handle = 2;
  out.handle   = 3;
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(double), &in[0]);
  ORC_SDK_DECK_INIT(in[0].items, double, (1.0));
  orc_sdk_oh_update(&in[0]);
  in[1].type_id   = ORC_TYPE_F64;
  in[1].item_size = 12;

  OrcError err = MAKE_VEC_INFO.func(0, in, 2, &out, 1);

  TEST_ASSERT_EQUAL_UINT64(ORC_ERROR_INVALID_ARGUMENTS, err);
  TEST_ASSERT_NULL(out.items);
  TEST_ASSERT_NULL(out.free_fn);
  orc_sdk_handle_free(&in[0]);
}

/* ============================================================
   make_vec — Ownership and lifetime invariants
   ============================================================ */

static void test_make_vec_output_free_fn_set(void)
{
  /* After a successful call, out.free_fn must be set (plugin owns the deck). */
  orc_sdk_init(NULL, NULL);
  OrcHandle in[2] = {{0}, {0}}, out = {0};
  in[0].handle = 1;
  in[1].handle = 2;
  out.handle   = 3;
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(double), &in[0]);
  ORC_SDK_DECK_INIT(in[0].items, double, (1.0));
  orc_sdk_oh_update(&in[0]);
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(double), &in[1]);
  ORC_SDK_DECK_INIT(in[1].items, double, (2.0));
  orc_sdk_oh_update(&in[1]);

  MAKE_VEC_INFO.func(0, in, 2, &out, 1);

  TEST_ASSERT_NOT_NULL(out.free_fn);
  orc_sdk_handle_free(&in[0]);
  orc_sdk_handle_free(&in[1]);
  orc_sdk_handle_free(&out);
}

static void test_make_vec_output_handle_preserved(void)
{
  /* out.handle must be unchanged before and after the call. */
  orc_sdk_init(NULL, NULL);
  OrcHandle in[2] = {{0}, {0}}, out = {0};
  in[0].handle = 1;
  in[1].handle = 2;
  out.handle   = 99;
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(double), &in[0]);
  ORC_SDK_DECK_INIT(in[0].items, double, (1.0));
  orc_sdk_oh_update(&in[0]);
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(double), &in[1]);
  ORC_SDK_DECK_INIT(in[1].items, double, (2.0));
  orc_sdk_oh_update(&in[1]);

  MAKE_VEC_INFO.func(0, in, 2, &out, 1);

  TEST_ASSERT_EQUAL_UINT64(99, out.handle);
  orc_sdk_handle_free(&in[0]);
  orc_sdk_handle_free(&in[1]);
  orc_sdk_handle_free(&out);
}

static void test_make_vec_input_handles_unaffected(void)
{
  /* The input handles' items pointers and n_items must not change. */
  orc_sdk_init(NULL, NULL);
  OrcHandle in[2] = {{0}, {0}}, out = {0};
  in[0].handle = 1;
  in[1].handle = 2;
  out.handle   = 3;
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(double), &in[0]);
  ORC_SDK_DECK_INIT(in[0].items, double, (1.0, 2.0));
  orc_sdk_oh_update(&in[0]);
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(double), &in[1]);
  ORC_SDK_DECK_INIT(in[1].items, double, (3.0, 4.0));
  orc_sdk_oh_update(&in[1]);
  void const *items0_before   = in[0].items;
  uint64_t    n_items0_before = in[0].n_items;
  void const *items1_before   = in[1].items;
  uint64_t    n_items1_before = in[1].n_items;

  MAKE_VEC_INFO.func(0, in, 2, &out, 1);

  TEST_ASSERT_EQUAL_PTR(items0_before, in[0].items);
  TEST_ASSERT_EQUAL_UINT64(n_items0_before, in[0].n_items);
  TEST_ASSERT_EQUAL_PTR(items1_before, in[1].items);
  TEST_ASSERT_EQUAL_UINT64(n_items1_before, in[1].n_items);
  orc_sdk_handle_free(&in[0]);
  orc_sdk_handle_free(&in[1]);
  orc_sdk_handle_free(&out);
}

static void test_make_vec_reuse_output_same_type(void)
{
  /* Calling make_vec twice with the same output type/arity reuses the deck instead of
   * reallocating -- same item count both times, so the initial capacity suffices. */
  orc_sdk_init(NULL, NULL);
  OrcHandle in[2] = {{0}, {0}}, out = {0};
  in[0].handle = 1;
  in[1].handle = 2;
  out.handle   = 3;
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(double), &in[0]);
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(double), &in[1]);

  ORC_SDK_DECK_INIT(in[0].items, double, (1.0));
  orc_sdk_oh_update(&in[0]);
  ORC_SDK_DECK_INIT(in[1].items, double, (2.0));
  orc_sdk_oh_update(&in[1]);
  MAKE_VEC_INFO.func(0, in, 2, &out, 1);
  void const *ptr_after_first = out.items;

  ORC_SDK_DECK_INIT(in[0].items, double, (5.0));
  orc_sdk_oh_update(&in[0]);
  ORC_SDK_DECK_INIT(in[1].items, double, (6.0));
  orc_sdk_oh_update(&in[1]);
  MAKE_VEC_INFO.func(0, in, 2, &out, 1);

  TEST_ASSERT_EQUAL_UINT64(1, out.n_items);
  double const *result = (double const *)out.items;
  TEST_ASSERT_EQUAL_DOUBLE(5.0, result[0]);
  TEST_ASSERT_EQUAL_DOUBLE(6.0, result[1]);
  TEST_ASSERT_EQUAL_PTR(ptr_after_first, out.items);

  orc_sdk_handle_free(&in[0]);
  orc_sdk_handle_free(&in[1]);
  orc_sdk_handle_free(&out);
}

static void test_make_vec_output_type_change(void)
{
  /* If out was previously allocated for a different component type, make_vec reallocates
   * it instead of reusing the stale deck. */
  orc_sdk_init(NULL, NULL);
  OrcHandle in[2] = {{0}, {0}}, out = {0};
  in[0].handle = 1;
  in[1].handle = 2;
  out.handle   = 3;
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(double), &in[0]);
  ORC_SDK_DECK_INIT(in[0].items, double, (1.0));
  orc_sdk_oh_update(&in[0]);
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(double), &in[1]);
  ORC_SDK_DECK_INIT(in[1].items, double, (2.0));
  orc_sdk_oh_update(&in[1]);
  MAKE_VEC_INFO.func(0, in, 2, &out, 1);
  TEST_ASSERT_EQUAL_UINT64(ORC_TYPE_F64, out.type_id);
  orc_sdk_handle_free(&in[0]);
  orc_sdk_handle_free(&in[1]);

  OrcHandle in2[2] = {{0}, {0}};
  in2[0].handle    = 4;
  in2[1].handle    = 5;
  orc_sdk_handle_alloc(ORC_TYPE_I32, sizeof(int32_t), &in2[0]);
  ORC_SDK_DECK_INIT(in2[0].items, int32_t, (7));
  orc_sdk_oh_update(&in2[0]);
  orc_sdk_handle_alloc(ORC_TYPE_I32, sizeof(int32_t), &in2[1]);
  ORC_SDK_DECK_INIT(in2[1].items, int32_t, (8));
  orc_sdk_oh_update(&in2[1]);

  MAKE_VEC_INFO.func(0, in2, 2, &out, 1);

  TEST_ASSERT_EQUAL_UINT64(ORC_TYPE_I32, out.type_id);
  TEST_ASSERT_EQUAL_UINT64(2 * sizeof(int32_t), out.item_size);
  int32_t const *result = (int32_t const *)out.items;
  TEST_ASSERT_EQUAL_INT32(7, result[0]);
  TEST_ASSERT_EQUAL_INT32(8, result[1]);
  orc_sdk_handle_free(&in2[0]);
  orc_sdk_handle_free(&in2[1]);
  orc_sdk_handle_free(&out);
}

/* ============================================================
   vec_add — Correctness
   ============================================================ */

static void test_vec_add_two_dvec2(void)
{
  /* (1,2) + (10,20) -> (11,22). */
  orc_sdk_init(NULL, NULL);
  OrcHandle in[2] = {{0}, {0}}, out = {0};
  in[0].handle = 1;
  in[1].handle = 2;
  out.handle   = 3;
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(_Vec2), &in[0]);
  _Vec2 *v0 = (_Vec2 *)in[0].items;
  orc_sdk_deck_push(v0, ((_Vec2) {1.0, 2.0}), 0);
  in[0].items = v0;
  orc_sdk_oh_update(&in[0]);
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(_Vec2), &in[1]);
  _Vec2 *v1 = (_Vec2 *)in[1].items;
  orc_sdk_deck_push(v1, ((_Vec2) {10.0, 20.0}), 0);
  in[1].items = v1;
  orc_sdk_oh_update(&in[1]);

  OrcError err = VEC_ADD_INFO.func(0, in, 2, &out, 1);

  TEST_ASSERT_EQUAL_UINT64(ORC_ERROR_NONE, err);
  TEST_ASSERT_EQUAL_UINT64(ORC_TYPE_F64, out.type_id);
  TEST_ASSERT_EQUAL_UINT64(sizeof(_Vec2), out.item_size);
  TEST_ASSERT_EQUAL_UINT64(1, out.n_items);
  double const *result = (double const *)out.items;
  TEST_ASSERT_EQUAL_DOUBLE(11.0, result[0]);
  TEST_ASSERT_EQUAL_DOUBLE(22.0, result[1]);
  orc_sdk_handle_free(&in[0]);
  orc_sdk_handle_free(&in[1]);
  orc_sdk_handle_free(&out);
}

static void test_vec_add_three_dvec3(void)
{
  /* (1,2,3) + (10,20,30) + (100,200,300) -> (111,222,333). N-ary fold, not just 2. */
  orc_sdk_init(NULL, NULL);
  OrcHandle in[3] = {{0}, {0}, {0}}, out = {0};
  in[0].handle = 1;
  in[1].handle = 2;
  in[2].handle = 3;
  out.handle   = 4;
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(_Vec3), &in[0]);
  _Vec3 *v0 = (_Vec3 *)in[0].items;
  orc_sdk_deck_push(v0, ((_Vec3) {1.0, 2.0, 3.0}), 0);
  in[0].items = v0;
  orc_sdk_oh_update(&in[0]);
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(_Vec3), &in[1]);
  _Vec3 *v1 = (_Vec3 *)in[1].items;
  orc_sdk_deck_push(v1, ((_Vec3) {10.0, 20.0, 30.0}), 0);
  in[1].items = v1;
  orc_sdk_oh_update(&in[1]);
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(_Vec3), &in[2]);
  _Vec3 *v2 = (_Vec3 *)in[2].items;
  orc_sdk_deck_push(v2, ((_Vec3) {100.0, 200.0, 300.0}), 0);
  in[2].items = v2;
  orc_sdk_oh_update(&in[2]);

  OrcError err = VEC_ADD_INFO.func(0, in, 3, &out, 1);

  TEST_ASSERT_EQUAL_UINT64(ORC_ERROR_NONE, err);
  TEST_ASSERT_EQUAL_UINT64(sizeof(_Vec3), out.item_size);
  TEST_ASSERT_EQUAL_UINT64(1, out.n_items);
  double const *result = (double const *)out.items;
  TEST_ASSERT_EQUAL_DOUBLE(111.0, result[0]);
  TEST_ASSERT_EQUAL_DOUBLE(222.0, result[1]);
  TEST_ASSERT_EQUAL_DOUBLE(333.0, result[2]);
  orc_sdk_handle_free(&in[0]);
  orc_sdk_handle_free(&in[1]);
  orc_sdk_handle_free(&in[2]);
  orc_sdk_handle_free(&out);
}

static void test_vec_add_multi_row_broadcast(void)
{
  /* Two decks of 3 dvec2 rows each -> elementwise sum per row. */
  orc_sdk_init(NULL, NULL);
  OrcHandle in[2] = {{0}, {0}}, out = {0};
  in[0].handle = 1;
  in[1].handle = 2;
  out.handle   = 3;
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(_Vec2), &in[0]);
  _Vec2 *v0 = (_Vec2 *)in[0].items;
  orc_sdk_deck_push(v0, ((_Vec2) {1.0, 2.0}), 0);
  orc_sdk_deck_push(v0, ((_Vec2) {3.0, 4.0}), 0);
  orc_sdk_deck_push(v0, ((_Vec2) {5.0, 6.0}), 0);
  in[0].items = v0;
  orc_sdk_oh_update(&in[0]);
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(_Vec2), &in[1]);
  _Vec2 *v1 = (_Vec2 *)in[1].items;
  orc_sdk_deck_push(v1, ((_Vec2) {10.0, 20.0}), 0);
  orc_sdk_deck_push(v1, ((_Vec2) {30.0, 40.0}), 0);
  orc_sdk_deck_push(v1, ((_Vec2) {50.0, 60.0}), 0);
  in[1].items = v1;
  orc_sdk_oh_update(&in[1]);

  OrcError err = VEC_ADD_INFO.func(0, in, 2, &out, 1);

  TEST_ASSERT_EQUAL_UINT64(ORC_ERROR_NONE, err);
  TEST_ASSERT_EQUAL_UINT64(3, out.n_items);
  TEST_ASSERT_EQUAL_UINT64(sizeof(_Vec2), out.item_size);
  _Vec2 const *result = (_Vec2 const *)out.items;
  TEST_ASSERT_EQUAL_DOUBLE(11.0, result[0].x);
  TEST_ASSERT_EQUAL_DOUBLE(22.0, result[0].y);
  TEST_ASSERT_EQUAL_DOUBLE(33.0, result[1].x);
  TEST_ASSERT_EQUAL_DOUBLE(44.0, result[1].y);
  TEST_ASSERT_EQUAL_DOUBLE(55.0, result[2].x);
  TEST_ASSERT_EQUAL_DOUBLE(66.0, result[2].y);
  orc_sdk_handle_free(&in[0]);
  orc_sdk_handle_free(&in[1]);
  orc_sdk_handle_free(&out);
}

static void test_vec_add_integer_type(void)
{
  /* Works for i32, not just f64. (7) + (-3) -> (4). */
  orc_sdk_init(NULL, NULL);
  OrcHandle in[2] = {{0}, {0}}, out = {0};
  in[0].handle = 1;
  in[1].handle = 2;
  out.handle   = 3;
  orc_sdk_handle_alloc(ORC_TYPE_I32, sizeof(int32_t), &in[0]);
  ORC_SDK_DECK_INIT(in[0].items, int32_t, (7));
  orc_sdk_oh_update(&in[0]);
  orc_sdk_handle_alloc(ORC_TYPE_I32, sizeof(int32_t), &in[1]);
  ORC_SDK_DECK_INIT(in[1].items, int32_t, (-3));
  orc_sdk_oh_update(&in[1]);

  OrcError err = VEC_ADD_INFO.func(0, in, 2, &out, 1);

  TEST_ASSERT_EQUAL_UINT64(ORC_ERROR_NONE, err);
  TEST_ASSERT_EQUAL_UINT64(ORC_TYPE_I32, out.type_id);
  TEST_ASSERT_EQUAL_UINT64(sizeof(int32_t), out.item_size);
  int32_t const *result = (int32_t const *)out.items;
  TEST_ASSERT_EQUAL_INT32(4, result[0]);
  orc_sdk_handle_free(&in[0]);
  orc_sdk_handle_free(&in[1]);
  orc_sdk_handle_free(&out);
}

static void test_vec_add_uint8_type(void)
{
  /* Works for u8, not just signed/floating types -- e.g. rgb color channels. (10) +
     (20) -> (30). */
  orc_sdk_init(NULL, NULL);
  OrcHandle in[2] = {{0}, {0}}, out = {0};
  in[0].handle = 1;
  in[1].handle = 2;
  out.handle   = 3;
  orc_sdk_handle_alloc(ORC_TYPE_U8, sizeof(uint8_t), &in[0]);
  ORC_SDK_DECK_INIT(in[0].items, uint8_t, (10));
  orc_sdk_oh_update(&in[0]);
  orc_sdk_handle_alloc(ORC_TYPE_U8, sizeof(uint8_t), &in[1]);
  ORC_SDK_DECK_INIT(in[1].items, uint8_t, (20));
  orc_sdk_oh_update(&in[1]);

  OrcError err = VEC_ADD_INFO.func(0, in, 2, &out, 1);

  TEST_ASSERT_EQUAL_UINT64(ORC_ERROR_NONE, err);
  TEST_ASSERT_EQUAL_UINT64(ORC_TYPE_U8, out.type_id);
  TEST_ASSERT_EQUAL_UINT64(sizeof(uint8_t), out.item_size);
  uint8_t const *result = (uint8_t const *)out.items;
  TEST_ASSERT_EQUAL_UINT8(30, result[0]);
  orc_sdk_handle_free(&in[0]);
  orc_sdk_handle_free(&in[1]);
  orc_sdk_handle_free(&out);
}

static void test_vec_add_uint32_type(void)
{
  /* Works for u32 too. (100000) + (250000) -> (350000). */
  orc_sdk_init(NULL, NULL);
  OrcHandle in[2] = {{0}, {0}}, out = {0};
  in[0].handle = 1;
  in[1].handle = 2;
  out.handle   = 3;
  orc_sdk_handle_alloc(ORC_TYPE_U32, sizeof(uint32_t), &in[0]);
  ORC_SDK_DECK_INIT(in[0].items, uint32_t, (100000u));
  orc_sdk_oh_update(&in[0]);
  orc_sdk_handle_alloc(ORC_TYPE_U32, sizeof(uint32_t), &in[1]);
  ORC_SDK_DECK_INIT(in[1].items, uint32_t, (250000u));
  orc_sdk_oh_update(&in[1]);

  OrcError err = VEC_ADD_INFO.func(0, in, 2, &out, 1);

  TEST_ASSERT_EQUAL_UINT64(ORC_ERROR_NONE, err);
  TEST_ASSERT_EQUAL_UINT64(ORC_TYPE_U32, out.type_id);
  TEST_ASSERT_EQUAL_UINT64(sizeof(uint32_t), out.item_size);
  uint32_t const *result = (uint32_t const *)out.items;
  TEST_ASSERT_EQUAL_UINT32(350000u, result[0]);
  orc_sdk_handle_free(&in[0]);
  orc_sdk_handle_free(&in[1]);
  orc_sdk_handle_free(&out);
}

/* ============================================================
   vec_add — Error / validation
   ============================================================ */

static void test_vec_add_too_few_inputs(void)
{
  /* n_inputs < 2 -> early return, output untouched. */
  orc_sdk_init(NULL, NULL);
  OrcHandle in = {0}, out = {0};
  in.handle  = 1;
  out.handle = 2;
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(double), &in);
  ORC_SDK_DECK_INIT(in.items, double, (1.0));
  orc_sdk_oh_update(&in);

  OrcError err = VEC_ADD_INFO.func(0, &in, 1, &out, 1);

  TEST_ASSERT_EQUAL_UINT64(ORC_ERROR_INVALID_ARGUMENTS, err);
  TEST_ASSERT_NULL(out.items);
  TEST_ASSERT_NULL(out.free_fn);
  orc_sdk_handle_free(&in);
}

static void test_vec_add_wrong_n_outputs(void)
{
  /* n_outputs != 1 -> early return, output untouched. */
  orc_sdk_init(NULL, NULL);
  OrcHandle in[2] = {{0}, {0}}, out = {0};
  in[0].handle = 1;
  in[1].handle = 2;
  out.handle   = 3;
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(double), &in[0]);
  ORC_SDK_DECK_INIT(in[0].items, double, (1.0));
  orc_sdk_oh_update(&in[0]);
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(double), &in[1]);
  ORC_SDK_DECK_INIT(in[1].items, double, (2.0));
  orc_sdk_oh_update(&in[1]);

  OrcError err = VEC_ADD_INFO.func(0, in, 2, &out, 2);

  TEST_ASSERT_EQUAL_UINT64(ORC_ERROR_INVALID_ARGUMENTS, err);
  TEST_ASSERT_NULL(out.items);
  TEST_ASSERT_NULL(out.free_fn);
  orc_sdk_handle_free(&in[0]);
  orc_sdk_handle_free(&in[1]);
}

static void test_vec_add_rejects_non_primitive_type(void)
{
  /* Vector components must be primitive scalar types -- proxies are rejected. */
  orc_sdk_init(NULL, NULL);
  OrcHandle in[2] = {{0}, {0}}, out = {0};
  in[0].handle = 1;
  in[1].handle = 2;
  out.handle   = 3;
  orc_sdk_handle_alloc(ORC_TYPE_PROXY, sizeof(OrcItemProxy), &in[0]);
  orc_sdk_handle_alloc(ORC_TYPE_PROXY, sizeof(OrcItemProxy), &in[1]);

  OrcError err = VEC_ADD_INFO.func(0, in, 2, &out, 1);

  TEST_ASSERT_EQUAL_UINT64(ORC_ERROR_TYPE_MISMATCH, err);
  TEST_ASSERT_NULL(out.items);
  TEST_ASSERT_NULL(out.free_fn);
  orc_sdk_handle_free(&in[0]);
  orc_sdk_handle_free(&in[1]);
}

static void test_vec_add_rejects_mismatched_types(void)
{
  /* All inputs must be of the same type -- f64 mixed with i32 is rejected. */
  orc_sdk_init(NULL, NULL);
  OrcHandle in[2] = {{0}, {0}}, out = {0};
  in[0].handle = 1;
  in[1].handle = 2;
  out.handle   = 3;
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(double), &in[0]);
  ORC_SDK_DECK_INIT(in[0].items, double, (1.0));
  orc_sdk_oh_update(&in[0]);
  orc_sdk_handle_alloc(ORC_TYPE_I32, sizeof(int32_t), &in[1]);
  ORC_SDK_DECK_INIT(in[1].items, int32_t, (2));
  orc_sdk_oh_update(&in[1]);

  OrcError err = VEC_ADD_INFO.func(0, in, 2, &out, 1);

  TEST_ASSERT_EQUAL_UINT64(ORC_ERROR_INVALID_ARGUMENTS, err);
  TEST_ASSERT_NULL(out.items);
  TEST_ASSERT_NULL(out.free_fn);
  orc_sdk_handle_free(&in[0]);
  orc_sdk_handle_free(&in[1]);
}

static void test_vec_add_rejects_mismatched_arity(void)
{
  /* dvec2 + dvec3 -- same type_id, but different item_size (arity). Unlike make_vec,
     vec_add is elementwise, not concatenation, so it requires an exact match. */
  orc_sdk_init(NULL, NULL);
  OrcHandle in[2] = {{0}, {0}}, out = {0};
  in[0].handle = 1;
  in[1].handle = 2;
  out.handle   = 3;
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(_Vec2), &in[0]);
  _Vec2 *v0 = (_Vec2 *)in[0].items;
  orc_sdk_deck_push(v0, ((_Vec2) {1.0, 2.0}), 0);
  in[0].items = v0;
  orc_sdk_oh_update(&in[0]);
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(_Vec3), &in[1]);
  _Vec3 *v1 = (_Vec3 *)in[1].items;
  orc_sdk_deck_push(v1, ((_Vec3) {1.0, 2.0, 3.0}), 0);
  in[1].items = v1;
  orc_sdk_oh_update(&in[1]);

  OrcError err = VEC_ADD_INFO.func(0, in, 2, &out, 1);

  TEST_ASSERT_EQUAL_UINT64(ORC_ERROR_INVALID_ARGUMENTS, err);
  TEST_ASSERT_NULL(out.items);
  TEST_ASSERT_NULL(out.free_fn);
  orc_sdk_handle_free(&in[0]);
  orc_sdk_handle_free(&in[1]);
}

static void test_vec_add_rejects_zero_item_size(void)
{
  /* The first component has item_size == 0 -- invalid handle. */
  orc_sdk_init(NULL, NULL);
  OrcHandle in[2] = {{0}, {0}}, out = {0};
  in[0].handle  = 1;
  in[1].handle  = 2;
  out.handle    = 3;
  in[0].type_id = ORC_TYPE_F64; /* item_size left at 0 -- never allocated. */
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(double), &in[1]);
  ORC_SDK_DECK_INIT(in[1].items, double, (1.0));
  orc_sdk_oh_update(&in[1]);

  OrcError err = VEC_ADD_INFO.func(0, in, 2, &out, 1);

  TEST_ASSERT_EQUAL_UINT64(ORC_ERROR_INVALID_HANDLE, err);
  TEST_ASSERT_NULL(out.items);
  TEST_ASSERT_NULL(out.free_fn);
  orc_sdk_handle_free(&in[1]);
}

static void test_vec_add_rejects_invalid_aggregate_item_size(void)
{
  /* item_size == 12 is not a multiple of sizeof(double) == 8. Both inputs share this
     item_size so the arity-match check passes and the aggregate check is what fires.
     Constructed via direct field assignment: orc_sdk_handle_alloc itself already rejects
     item_size % scalar_size != 0, so it can't be used to create this handle -- we're
     testing vec_add's own defensive check against a handle produced some other way. */
  orc_sdk_init(NULL, NULL);
  OrcHandle in[2] = {{0}, {0}}, out = {0};
  in[0].handle    = 1;
  in[1].handle    = 2;
  out.handle      = 3;
  in[0].type_id   = ORC_TYPE_F64;
  in[0].item_size = 12;
  in[1].type_id   = ORC_TYPE_F64;
  in[1].item_size = 12;

  OrcError err = VEC_ADD_INFO.func(0, in, 2, &out, 1);

  TEST_ASSERT_EQUAL_UINT64(ORC_ERROR_INVALID_ARGUMENTS, err);
  TEST_ASSERT_NULL(out.items);
  TEST_ASSERT_NULL(out.free_fn);
}

/* ============================================================
   vec_add — Ownership and lifetime invariants
   ============================================================ */

static void test_vec_add_output_free_fn_set(void)
{
  /* After a successful call, out.free_fn must be set (plugin owns the deck). */
  orc_sdk_init(NULL, NULL);
  OrcHandle in[2] = {{0}, {0}}, out = {0};
  in[0].handle = 1;
  in[1].handle = 2;
  out.handle   = 3;
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(double), &in[0]);
  ORC_SDK_DECK_INIT(in[0].items, double, (1.0));
  orc_sdk_oh_update(&in[0]);
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(double), &in[1]);
  ORC_SDK_DECK_INIT(in[1].items, double, (2.0));
  orc_sdk_oh_update(&in[1]);

  VEC_ADD_INFO.func(0, in, 2, &out, 1);

  TEST_ASSERT_NOT_NULL(out.free_fn);
  orc_sdk_handle_free(&in[0]);
  orc_sdk_handle_free(&in[1]);
  orc_sdk_handle_free(&out);
}

static void test_vec_add_output_handle_preserved(void)
{
  /* out.handle must be unchanged before and after the call. */
  orc_sdk_init(NULL, NULL);
  OrcHandle in[2] = {{0}, {0}}, out = {0};
  in[0].handle = 1;
  in[1].handle = 2;
  out.handle   = 99;
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(double), &in[0]);
  ORC_SDK_DECK_INIT(in[0].items, double, (1.0));
  orc_sdk_oh_update(&in[0]);
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(double), &in[1]);
  ORC_SDK_DECK_INIT(in[1].items, double, (2.0));
  orc_sdk_oh_update(&in[1]);

  VEC_ADD_INFO.func(0, in, 2, &out, 1);

  TEST_ASSERT_EQUAL_UINT64(99, out.handle);
  orc_sdk_handle_free(&in[0]);
  orc_sdk_handle_free(&in[1]);
  orc_sdk_handle_free(&out);
}

static void test_vec_add_input_handles_unaffected(void)
{
  /* The input handles' items pointers and n_items must not change. */
  orc_sdk_init(NULL, NULL);
  OrcHandle in[2] = {{0}, {0}}, out = {0};
  in[0].handle = 1;
  in[1].handle = 2;
  out.handle   = 3;
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(double), &in[0]);
  ORC_SDK_DECK_INIT(in[0].items, double, (1.0, 2.0));
  orc_sdk_oh_update(&in[0]);
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(double), &in[1]);
  ORC_SDK_DECK_INIT(in[1].items, double, (3.0, 4.0));
  orc_sdk_oh_update(&in[1]);
  void const *items0_before   = in[0].items;
  uint64_t    n_items0_before = in[0].n_items;
  void const *items1_before   = in[1].items;
  uint64_t    n_items1_before = in[1].n_items;

  VEC_ADD_INFO.func(0, in, 2, &out, 1);

  TEST_ASSERT_EQUAL_PTR(items0_before, in[0].items);
  TEST_ASSERT_EQUAL_UINT64(n_items0_before, in[0].n_items);
  TEST_ASSERT_EQUAL_PTR(items1_before, in[1].items);
  TEST_ASSERT_EQUAL_UINT64(n_items1_before, in[1].n_items);
  orc_sdk_handle_free(&in[0]);
  orc_sdk_handle_free(&in[1]);
  orc_sdk_handle_free(&out);
}

static void test_vec_add_reuse_output_same_type(void)
{
  /* Calling vec_add twice with the same output type/arity reuses the deck instead of
   * reallocating -- same item count both times, so the initial capacity suffices. */
  orc_sdk_init(NULL, NULL);
  OrcHandle in[2] = {{0}, {0}}, out = {0};
  in[0].handle = 1;
  in[1].handle = 2;
  out.handle   = 3;
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(double), &in[0]);
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(double), &in[1]);

  ORC_SDK_DECK_INIT(in[0].items, double, (1.0));
  orc_sdk_oh_update(&in[0]);
  ORC_SDK_DECK_INIT(in[1].items, double, (2.0));
  orc_sdk_oh_update(&in[1]);
  VEC_ADD_INFO.func(0, in, 2, &out, 1);
  void const *ptr_after_first = out.items;

  ORC_SDK_DECK_INIT(in[0].items, double, (5.0));
  orc_sdk_oh_update(&in[0]);
  ORC_SDK_DECK_INIT(in[1].items, double, (6.0));
  orc_sdk_oh_update(&in[1]);
  VEC_ADD_INFO.func(0, in, 2, &out, 1);

  TEST_ASSERT_EQUAL_UINT64(1, out.n_items);
  double const *result = (double const *)out.items;
  TEST_ASSERT_EQUAL_DOUBLE(11.0, result[0]);
  TEST_ASSERT_EQUAL_PTR(ptr_after_first, out.items);

  orc_sdk_handle_free(&in[0]);
  orc_sdk_handle_free(&in[1]);
  orc_sdk_handle_free(&out);
}

static void test_vec_add_output_type_change(void)
{
  /* If out was previously allocated for a different component type, vec_add reallocates
   * it instead of reusing the stale deck. */
  orc_sdk_init(NULL, NULL);
  OrcHandle in[2] = {{0}, {0}}, out = {0};
  in[0].handle = 1;
  in[1].handle = 2;
  out.handle   = 3;
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(double), &in[0]);
  ORC_SDK_DECK_INIT(in[0].items, double, (1.0));
  orc_sdk_oh_update(&in[0]);
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(double), &in[1]);
  ORC_SDK_DECK_INIT(in[1].items, double, (2.0));
  orc_sdk_oh_update(&in[1]);
  VEC_ADD_INFO.func(0, in, 2, &out, 1);
  TEST_ASSERT_EQUAL_UINT64(ORC_TYPE_F64, out.type_id);
  orc_sdk_handle_free(&in[0]);
  orc_sdk_handle_free(&in[1]);

  OrcHandle in2[2] = {{0}, {0}};
  in2[0].handle = 4;
  in2[1].handle = 5;
  orc_sdk_handle_alloc(ORC_TYPE_I32, sizeof(int32_t), &in2[0]);
  ORC_SDK_DECK_INIT(in2[0].items, int32_t, (7));
  orc_sdk_oh_update(&in2[0]);
  orc_sdk_handle_alloc(ORC_TYPE_I32, sizeof(int32_t), &in2[1]);
  ORC_SDK_DECK_INIT(in2[1].items, int32_t, (8));
  orc_sdk_oh_update(&in2[1]);

  VEC_ADD_INFO.func(0, in2, 2, &out, 1);

  TEST_ASSERT_EQUAL_UINT64(ORC_TYPE_I32, out.type_id);
  TEST_ASSERT_EQUAL_UINT64(sizeof(int32_t), out.item_size);
  int32_t const *result = (int32_t const *)out.items;
  TEST_ASSERT_EQUAL_INT32(15, result[0]);
  orc_sdk_handle_free(&in2[0]);
  orc_sdk_handle_free(&in2[1]);
  orc_sdk_handle_free(&out);
}

/* ============================================================
   vec_subtract — Correctness
   ============================================================ */

static void test_vec_subtract_two_dvec2(void)
{
  /* (10,20) - (1,2) -> (9,18). Order-sensitive: first minus second. */
  orc_sdk_init(NULL, NULL);
  OrcHandle in[2] = {{0}, {0}}, out = {0};
  in[0].handle = 1;
  in[1].handle = 2;
  out.handle   = 3;
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(_Vec2), &in[0]);
  _Vec2 *v0 = (_Vec2 *)in[0].items;
  orc_sdk_deck_push(v0, ((_Vec2) {10.0, 20.0}), 0);
  in[0].items = v0;
  orc_sdk_oh_update(&in[0]);
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(_Vec2), &in[1]);
  _Vec2 *v1 = (_Vec2 *)in[1].items;
  orc_sdk_deck_push(v1, ((_Vec2) {1.0, 2.0}), 0);
  in[1].items = v1;
  orc_sdk_oh_update(&in[1]);

  OrcError err = VEC_SUBTRACT_INFO.func(0, in, 2, &out, 1);

  TEST_ASSERT_EQUAL_UINT64(ORC_ERROR_NONE, err);
  TEST_ASSERT_EQUAL_UINT64(ORC_TYPE_F64, out.type_id);
  TEST_ASSERT_EQUAL_UINT64(sizeof(_Vec2), out.item_size);
  TEST_ASSERT_EQUAL_UINT64(1, out.n_items);
  double const *result = (double const *)out.items;
  TEST_ASSERT_EQUAL_DOUBLE(9.0, result[0]);
  TEST_ASSERT_EQUAL_DOUBLE(18.0, result[1]);
  orc_sdk_handle_free(&in[0]);
  orc_sdk_handle_free(&in[1]);
  orc_sdk_handle_free(&out);
}

static void test_vec_subtract_multi_row_broadcast(void)
{
  /* Two decks of 3 dvec2 rows each -> elementwise subtract per row. */
  orc_sdk_init(NULL, NULL);
  OrcHandle in[2] = {{0}, {0}}, out = {0};
  in[0].handle = 1;
  in[1].handle = 2;
  out.handle   = 3;
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(_Vec2), &in[0]);
  _Vec2 *v0 = (_Vec2 *)in[0].items;
  orc_sdk_deck_push(v0, ((_Vec2) {10.0, 20.0}), 0);
  orc_sdk_deck_push(v0, ((_Vec2) {30.0, 40.0}), 0);
  orc_sdk_deck_push(v0, ((_Vec2) {50.0, 60.0}), 0);
  in[0].items = v0;
  orc_sdk_oh_update(&in[0]);
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(_Vec2), &in[1]);
  _Vec2 *v1 = (_Vec2 *)in[1].items;
  orc_sdk_deck_push(v1, ((_Vec2) {1.0, 2.0}), 0);
  orc_sdk_deck_push(v1, ((_Vec2) {3.0, 4.0}), 0);
  orc_sdk_deck_push(v1, ((_Vec2) {5.0, 6.0}), 0);
  in[1].items = v1;
  orc_sdk_oh_update(&in[1]);

  OrcError err = VEC_SUBTRACT_INFO.func(0, in, 2, &out, 1);

  TEST_ASSERT_EQUAL_UINT64(ORC_ERROR_NONE, err);
  TEST_ASSERT_EQUAL_UINT64(3, out.n_items);
  TEST_ASSERT_EQUAL_UINT64(sizeof(_Vec2), out.item_size);
  _Vec2 const *result = (_Vec2 const *)out.items;
  TEST_ASSERT_EQUAL_DOUBLE(9.0, result[0].x);
  TEST_ASSERT_EQUAL_DOUBLE(18.0, result[0].y);
  TEST_ASSERT_EQUAL_DOUBLE(27.0, result[1].x);
  TEST_ASSERT_EQUAL_DOUBLE(36.0, result[1].y);
  TEST_ASSERT_EQUAL_DOUBLE(45.0, result[2].x);
  TEST_ASSERT_EQUAL_DOUBLE(54.0, result[2].y);
  orc_sdk_handle_free(&in[0]);
  orc_sdk_handle_free(&in[1]);
  orc_sdk_handle_free(&out);
}

static void test_vec_subtract_integer_type(void)
{
  /* Works for i32, not just f64. (7) - (-3) -> (10). */
  orc_sdk_init(NULL, NULL);
  OrcHandle in[2] = {{0}, {0}}, out = {0};
  in[0].handle = 1;
  in[1].handle = 2;
  out.handle   = 3;
  orc_sdk_handle_alloc(ORC_TYPE_I32, sizeof(int32_t), &in[0]);
  ORC_SDK_DECK_INIT(in[0].items, int32_t, (7));
  orc_sdk_oh_update(&in[0]);
  orc_sdk_handle_alloc(ORC_TYPE_I32, sizeof(int32_t), &in[1]);
  ORC_SDK_DECK_INIT(in[1].items, int32_t, (-3));
  orc_sdk_oh_update(&in[1]);

  OrcError err = VEC_SUBTRACT_INFO.func(0, in, 2, &out, 1);

  TEST_ASSERT_EQUAL_UINT64(ORC_ERROR_NONE, err);
  TEST_ASSERT_EQUAL_UINT64(ORC_TYPE_I32, out.type_id);
  TEST_ASSERT_EQUAL_UINT64(sizeof(int32_t), out.item_size);
  int32_t const *result = (int32_t const *)out.items;
  TEST_ASSERT_EQUAL_INT32(10, result[0]);
  orc_sdk_handle_free(&in[0]);
  orc_sdk_handle_free(&in[1]);
  orc_sdk_handle_free(&out);
}

static void test_vec_subtract_uint8_type(void)
{
  /* Works for u8, not just signed/floating types -- e.g. rgb color channels. (200) -
     (50) -> (150). Stays within uint8_t range, no wraparound. */
  orc_sdk_init(NULL, NULL);
  OrcHandle in[2] = {{0}, {0}}, out = {0};
  in[0].handle = 1;
  in[1].handle = 2;
  out.handle   = 3;
  orc_sdk_handle_alloc(ORC_TYPE_U8, sizeof(uint8_t), &in[0]);
  ORC_SDK_DECK_INIT(in[0].items, uint8_t, (200));
  orc_sdk_oh_update(&in[0]);
  orc_sdk_handle_alloc(ORC_TYPE_U8, sizeof(uint8_t), &in[1]);
  ORC_SDK_DECK_INIT(in[1].items, uint8_t, (50));
  orc_sdk_oh_update(&in[1]);

  OrcError err = VEC_SUBTRACT_INFO.func(0, in, 2, &out, 1);

  TEST_ASSERT_EQUAL_UINT64(ORC_ERROR_NONE, err);
  TEST_ASSERT_EQUAL_UINT64(ORC_TYPE_U8, out.type_id);
  TEST_ASSERT_EQUAL_UINT64(sizeof(uint8_t), out.item_size);
  uint8_t const *result = (uint8_t const *)out.items;
  TEST_ASSERT_EQUAL_UINT8(150, result[0]);
  orc_sdk_handle_free(&in[0]);
  orc_sdk_handle_free(&in[1]);
  orc_sdk_handle_free(&out);
}

static void test_vec_subtract_uint32_type(void)
{
  /* Works for u32 too. (500000) - (200000) -> (300000). */
  orc_sdk_init(NULL, NULL);
  OrcHandle in[2] = {{0}, {0}}, out = {0};
  in[0].handle = 1;
  in[1].handle = 2;
  out.handle   = 3;
  orc_sdk_handle_alloc(ORC_TYPE_U32, sizeof(uint32_t), &in[0]);
  ORC_SDK_DECK_INIT(in[0].items, uint32_t, (500000u));
  orc_sdk_oh_update(&in[0]);
  orc_sdk_handle_alloc(ORC_TYPE_U32, sizeof(uint32_t), &in[1]);
  ORC_SDK_DECK_INIT(in[1].items, uint32_t, (200000u));
  orc_sdk_oh_update(&in[1]);

  OrcError err = VEC_SUBTRACT_INFO.func(0, in, 2, &out, 1);

  TEST_ASSERT_EQUAL_UINT64(ORC_ERROR_NONE, err);
  TEST_ASSERT_EQUAL_UINT64(ORC_TYPE_U32, out.type_id);
  TEST_ASSERT_EQUAL_UINT64(sizeof(uint32_t), out.item_size);
  uint32_t const *result = (uint32_t const *)out.items;
  TEST_ASSERT_EQUAL_UINT32(300000u, result[0]);
  orc_sdk_handle_free(&in[0]);
  orc_sdk_handle_free(&in[1]);
  orc_sdk_handle_free(&out);
}

/* ============================================================
   vec_subtract — Error / validation
   ============================================================ */

static void test_vec_subtract_wrong_n_inputs(void)
{
  /* n_inputs != 2 -> early return, output untouched. */
  orc_sdk_init(NULL, NULL);
  OrcHandle in = {0}, out = {0};
  in.handle  = 1;
  out.handle = 2;
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(double), &in);
  ORC_SDK_DECK_INIT(in.items, double, (1.0));
  orc_sdk_oh_update(&in);

  OrcError err = VEC_SUBTRACT_INFO.func(0, &in, 1, &out, 1);

  TEST_ASSERT_EQUAL_UINT64(ORC_ERROR_INVALID_ARGUMENTS, err);
  TEST_ASSERT_NULL(out.items);
  TEST_ASSERT_NULL(out.free_fn);
  orc_sdk_handle_free(&in);
}

static void test_vec_subtract_wrong_n_outputs(void)
{
  /* n_outputs != 1 -> early return, output untouched. */
  orc_sdk_init(NULL, NULL);
  OrcHandle in[2] = {{0}, {0}}, out = {0};
  in[0].handle = 1;
  in[1].handle = 2;
  out.handle   = 3;
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(double), &in[0]);
  ORC_SDK_DECK_INIT(in[0].items, double, (1.0));
  orc_sdk_oh_update(&in[0]);
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(double), &in[1]);
  ORC_SDK_DECK_INIT(in[1].items, double, (2.0));
  orc_sdk_oh_update(&in[1]);

  OrcError err = VEC_SUBTRACT_INFO.func(0, in, 2, &out, 2);

  TEST_ASSERT_EQUAL_UINT64(ORC_ERROR_INVALID_ARGUMENTS, err);
  TEST_ASSERT_NULL(out.items);
  TEST_ASSERT_NULL(out.free_fn);
  orc_sdk_handle_free(&in[0]);
  orc_sdk_handle_free(&in[1]);
}

static void test_vec_subtract_rejects_non_primitive_type(void)
{
  /* Vector components must be primitive scalar types -- proxies are rejected. */
  orc_sdk_init(NULL, NULL);
  OrcHandle in[2] = {{0}, {0}}, out = {0};
  in[0].handle = 1;
  in[1].handle = 2;
  out.handle   = 3;
  orc_sdk_handle_alloc(ORC_TYPE_PROXY, sizeof(OrcItemProxy), &in[0]);
  orc_sdk_handle_alloc(ORC_TYPE_PROXY, sizeof(OrcItemProxy), &in[1]);

  OrcError err = VEC_SUBTRACT_INFO.func(0, in, 2, &out, 1);

  TEST_ASSERT_EQUAL_UINT64(ORC_ERROR_TYPE_MISMATCH, err);
  TEST_ASSERT_NULL(out.items);
  TEST_ASSERT_NULL(out.free_fn);
  orc_sdk_handle_free(&in[0]);
  orc_sdk_handle_free(&in[1]);
}

static void test_vec_subtract_rejects_mismatched_types(void)
{
  /* Both inputs must be of the same type -- f64 mixed with i32 is rejected. */
  orc_sdk_init(NULL, NULL);
  OrcHandle in[2] = {{0}, {0}}, out = {0};
  in[0].handle = 1;
  in[1].handle = 2;
  out.handle   = 3;
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(double), &in[0]);
  ORC_SDK_DECK_INIT(in[0].items, double, (1.0));
  orc_sdk_oh_update(&in[0]);
  orc_sdk_handle_alloc(ORC_TYPE_I32, sizeof(int32_t), &in[1]);
  ORC_SDK_DECK_INIT(in[1].items, int32_t, (2));
  orc_sdk_oh_update(&in[1]);

  OrcError err = VEC_SUBTRACT_INFO.func(0, in, 2, &out, 1);

  TEST_ASSERT_EQUAL_UINT64(ORC_ERROR_INVALID_ARGUMENTS, err);
  TEST_ASSERT_NULL(out.items);
  TEST_ASSERT_NULL(out.free_fn);
  orc_sdk_handle_free(&in[0]);
  orc_sdk_handle_free(&in[1]);
}

static void test_vec_subtract_rejects_mismatched_arity(void)
{
  /* dvec2 - dvec3 -- same type_id, but different item_size (arity). Subtraction is
     elementwise, so it requires an exact match. */
  orc_sdk_init(NULL, NULL);
  OrcHandle in[2] = {{0}, {0}}, out = {0};
  in[0].handle = 1;
  in[1].handle = 2;
  out.handle   = 3;
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(_Vec2), &in[0]);
  _Vec2 *v0 = (_Vec2 *)in[0].items;
  orc_sdk_deck_push(v0, ((_Vec2) {1.0, 2.0}), 0);
  in[0].items = v0;
  orc_sdk_oh_update(&in[0]);
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(_Vec3), &in[1]);
  _Vec3 *v1 = (_Vec3 *)in[1].items;
  orc_sdk_deck_push(v1, ((_Vec3) {1.0, 2.0, 3.0}), 0);
  in[1].items = v1;
  orc_sdk_oh_update(&in[1]);

  OrcError err = VEC_SUBTRACT_INFO.func(0, in, 2, &out, 1);

  TEST_ASSERT_EQUAL_UINT64(ORC_ERROR_INVALID_ARGUMENTS, err);
  TEST_ASSERT_NULL(out.items);
  TEST_ASSERT_NULL(out.free_fn);
  orc_sdk_handle_free(&in[0]);
  orc_sdk_handle_free(&in[1]);
}

static void test_vec_subtract_rejects_zero_item_size(void)
{
  /* The first component has item_size == 0 -- invalid handle. */
  orc_sdk_init(NULL, NULL);
  OrcHandle in[2] = {{0}, {0}}, out = {0};
  in[0].handle  = 1;
  in[1].handle  = 2;
  out.handle    = 3;
  in[0].type_id = ORC_TYPE_F64; /* item_size left at 0 -- never allocated. */
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(double), &in[1]);
  ORC_SDK_DECK_INIT(in[1].items, double, (1.0));
  orc_sdk_oh_update(&in[1]);

  OrcError err = VEC_SUBTRACT_INFO.func(0, in, 2, &out, 1);

  TEST_ASSERT_EQUAL_UINT64(ORC_ERROR_INVALID_HANDLE, err);
  TEST_ASSERT_NULL(out.items);
  TEST_ASSERT_NULL(out.free_fn);
  orc_sdk_handle_free(&in[1]);
}

static void test_vec_subtract_rejects_invalid_aggregate_item_size(void)
{
  /* item_size == 12 is not a multiple of sizeof(double) == 8. Both inputs share this
     item_size so the arity-match check passes and the aggregate check is what fires.
     Constructed via direct field assignment: orc_sdk_handle_alloc itself already rejects
     item_size % scalar_size != 0, so it can't be used to create this handle. */
  orc_sdk_init(NULL, NULL);
  OrcHandle in[2] = {{0}, {0}}, out = {0};
  in[0].handle    = 1;
  in[1].handle    = 2;
  out.handle      = 3;
  in[0].type_id   = ORC_TYPE_F64;
  in[0].item_size = 12;
  in[1].type_id   = ORC_TYPE_F64;
  in[1].item_size = 12;

  OrcError err = VEC_SUBTRACT_INFO.func(0, in, 2, &out, 1);

  TEST_ASSERT_EQUAL_UINT64(ORC_ERROR_INVALID_ARGUMENTS, err);
  TEST_ASSERT_NULL(out.items);
  TEST_ASSERT_NULL(out.free_fn);
}

/* ============================================================
   vec_subtract — Ownership and lifetime invariants
   ============================================================ */

static void test_vec_subtract_output_free_fn_set(void)
{
  /* After a successful call, out.free_fn must be set (plugin owns the deck). */
  orc_sdk_init(NULL, NULL);
  OrcHandle in[2] = {{0}, {0}}, out = {0};
  in[0].handle = 1;
  in[1].handle = 2;
  out.handle   = 3;
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(double), &in[0]);
  ORC_SDK_DECK_INIT(in[0].items, double, (1.0));
  orc_sdk_oh_update(&in[0]);
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(double), &in[1]);
  ORC_SDK_DECK_INIT(in[1].items, double, (2.0));
  orc_sdk_oh_update(&in[1]);

  VEC_SUBTRACT_INFO.func(0, in, 2, &out, 1);

  TEST_ASSERT_NOT_NULL(out.free_fn);
  orc_sdk_handle_free(&in[0]);
  orc_sdk_handle_free(&in[1]);
  orc_sdk_handle_free(&out);
}

static void test_vec_subtract_output_handle_preserved(void)
{
  /* out.handle must be unchanged before and after the call. */
  orc_sdk_init(NULL, NULL);
  OrcHandle in[2] = {{0}, {0}}, out = {0};
  in[0].handle = 1;
  in[1].handle = 2;
  out.handle   = 99;
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(double), &in[0]);
  ORC_SDK_DECK_INIT(in[0].items, double, (1.0));
  orc_sdk_oh_update(&in[0]);
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(double), &in[1]);
  ORC_SDK_DECK_INIT(in[1].items, double, (2.0));
  orc_sdk_oh_update(&in[1]);

  VEC_SUBTRACT_INFO.func(0, in, 2, &out, 1);

  TEST_ASSERT_EQUAL_UINT64(99, out.handle);
  orc_sdk_handle_free(&in[0]);
  orc_sdk_handle_free(&in[1]);
  orc_sdk_handle_free(&out);
}

static void test_vec_subtract_input_handles_unaffected(void)
{
  /* The input handles' items pointers and n_items must not change. */
  orc_sdk_init(NULL, NULL);
  OrcHandle in[2] = {{0}, {0}}, out = {0};
  in[0].handle = 1;
  in[1].handle = 2;
  out.handle   = 3;
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(double), &in[0]);
  ORC_SDK_DECK_INIT(in[0].items, double, (1.0, 2.0));
  orc_sdk_oh_update(&in[0]);
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(double), &in[1]);
  ORC_SDK_DECK_INIT(in[1].items, double, (3.0, 4.0));
  orc_sdk_oh_update(&in[1]);
  void const *items0_before   = in[0].items;
  uint64_t    n_items0_before = in[0].n_items;
  void const *items1_before   = in[1].items;
  uint64_t    n_items1_before = in[1].n_items;

  VEC_SUBTRACT_INFO.func(0, in, 2, &out, 1);

  TEST_ASSERT_EQUAL_PTR(items0_before, in[0].items);
  TEST_ASSERT_EQUAL_UINT64(n_items0_before, in[0].n_items);
  TEST_ASSERT_EQUAL_PTR(items1_before, in[1].items);
  TEST_ASSERT_EQUAL_UINT64(n_items1_before, in[1].n_items);
  orc_sdk_handle_free(&in[0]);
  orc_sdk_handle_free(&in[1]);
  orc_sdk_handle_free(&out);
}

static void test_vec_subtract_reuse_output_same_type(void)
{
  /* Calling vec_subtract twice with the same output type/arity reuses the deck instead of
   * reallocating -- same item count both times, so the initial capacity suffices. */
  orc_sdk_init(NULL, NULL);
  OrcHandle in[2] = {{0}, {0}}, out = {0};
  in[0].handle = 1;
  in[1].handle = 2;
  out.handle   = 3;
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(double), &in[0]);
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(double), &in[1]);

  ORC_SDK_DECK_INIT(in[0].items, double, (10.0));
  orc_sdk_oh_update(&in[0]);
  ORC_SDK_DECK_INIT(in[1].items, double, (1.0));
  orc_sdk_oh_update(&in[1]);
  VEC_SUBTRACT_INFO.func(0, in, 2, &out, 1);
  void const *ptr_after_first = out.items;

  ORC_SDK_DECK_INIT(in[0].items, double, (20.0));
  orc_sdk_oh_update(&in[0]);
  ORC_SDK_DECK_INIT(in[1].items, double, (5.0));
  orc_sdk_oh_update(&in[1]);
  VEC_SUBTRACT_INFO.func(0, in, 2, &out, 1);

  TEST_ASSERT_EQUAL_UINT64(1, out.n_items);
  double const *result = (double const *)out.items;
  TEST_ASSERT_EQUAL_DOUBLE(15.0, result[0]);
  TEST_ASSERT_EQUAL_PTR(ptr_after_first, out.items);

  orc_sdk_handle_free(&in[0]);
  orc_sdk_handle_free(&in[1]);
  orc_sdk_handle_free(&out);
}

static void test_vec_subtract_output_type_change(void)
{
  /* If out was previously allocated for a different component type, vec_subtract
   * reallocates it instead of reusing the stale deck. */
  orc_sdk_init(NULL, NULL);
  OrcHandle in[2] = {{0}, {0}}, out = {0};
  in[0].handle = 1;
  in[1].handle = 2;
  out.handle   = 3;
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(double), &in[0]);
  ORC_SDK_DECK_INIT(in[0].items, double, (1.0));
  orc_sdk_oh_update(&in[0]);
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(double), &in[1]);
  ORC_SDK_DECK_INIT(in[1].items, double, (2.0));
  orc_sdk_oh_update(&in[1]);
  VEC_SUBTRACT_INFO.func(0, in, 2, &out, 1);
  TEST_ASSERT_EQUAL_UINT64(ORC_TYPE_F64, out.type_id);
  orc_sdk_handle_free(&in[0]);
  orc_sdk_handle_free(&in[1]);

  OrcHandle in2[2] = {{0}, {0}};
  in2[0].handle = 4;
  in2[1].handle = 5;
  orc_sdk_handle_alloc(ORC_TYPE_I32, sizeof(int32_t), &in2[0]);
  ORC_SDK_DECK_INIT(in2[0].items, int32_t, (7));
  orc_sdk_oh_update(&in2[0]);
  orc_sdk_handle_alloc(ORC_TYPE_I32, sizeof(int32_t), &in2[1]);
  ORC_SDK_DECK_INIT(in2[1].items, int32_t, (3));
  orc_sdk_oh_update(&in2[1]);

  VEC_SUBTRACT_INFO.func(0, in2, 2, &out, 1);

  TEST_ASSERT_EQUAL_UINT64(ORC_TYPE_I32, out.type_id);
  TEST_ASSERT_EQUAL_UINT64(sizeof(int32_t), out.item_size);
  int32_t const *result = (int32_t const *)out.items;
  TEST_ASSERT_EQUAL_INT32(4, result[0]);
  orc_sdk_handle_free(&in2[0]);
  orc_sdk_handle_free(&in2[1]);
  orc_sdk_handle_free(&out);
}

/* ============================================================
   vec_dot_product — Correctness
   ============================================================ */

static void test_vec_dot_product_two_dvec2_f64(void)
{
  /* (1,2) . (3,4) -> 1*3 + 2*4 = 11. Output is a single scalar, not a vector. */
  orc_sdk_init(NULL, NULL);
  OrcHandle in[2] = {{0}, {0}}, out = {0};
  in[0].handle = 1;
  in[1].handle = 2;
  out.handle   = 3;
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(_Vec2), &in[0]);
  _Vec2 *v0 = (_Vec2 *)in[0].items;
  orc_sdk_deck_push(v0, ((_Vec2) {1.0, 2.0}), 0);
  in[0].items = v0;
  orc_sdk_oh_update(&in[0]);
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(_Vec2), &in[1]);
  _Vec2 *v1 = (_Vec2 *)in[1].items;
  orc_sdk_deck_push(v1, ((_Vec2) {3.0, 4.0}), 0);
  in[1].items = v1;
  orc_sdk_oh_update(&in[1]);

  OrcError err = VEC_DOT_PRODUCT_INFO.func(0, in, 2, &out, 1);

  TEST_ASSERT_EQUAL_UINT64(ORC_ERROR_NONE, err);
  TEST_ASSERT_EQUAL_UINT64(ORC_TYPE_F64, out.type_id);
  TEST_ASSERT_EQUAL_UINT64(sizeof(double), out.item_size);
  TEST_ASSERT_EQUAL_UINT64(1, out.n_items);
  double const *result = (double const *)out.items;
  TEST_ASSERT_EQUAL_DOUBLE(11.0, result[0]);
  orc_sdk_handle_free(&in[0]);
  orc_sdk_handle_free(&in[1]);
  orc_sdk_handle_free(&out);
}

static void test_vec_dot_product_two_vec3_f32(void)
{
  /* (1,2,3) . (4,5,6) -> 4 + 10 + 18 = 32. Confirms float works, not just double. */
  orc_sdk_init(NULL, NULL);
  OrcHandle in[2] = {{0}, {0}}, out = {0};
  in[0].handle = 1;
  in[1].handle = 2;
  out.handle   = 3;
  orc_sdk_handle_alloc(ORC_TYPE_F32, sizeof(_FVec3), &in[0]);
  _FVec3 *v0 = (_FVec3 *)in[0].items;
  orc_sdk_deck_push(v0, ((_FVec3) {1.0f, 2.0f, 3.0f}), 0);
  in[0].items = v0;
  orc_sdk_oh_update(&in[0]);
  orc_sdk_handle_alloc(ORC_TYPE_F32, sizeof(_FVec3), &in[1]);
  _FVec3 *v1 = (_FVec3 *)in[1].items;
  orc_sdk_deck_push(v1, ((_FVec3) {4.0f, 5.0f, 6.0f}), 0);
  in[1].items = v1;
  orc_sdk_oh_update(&in[1]);

  OrcError err = VEC_DOT_PRODUCT_INFO.func(0, in, 2, &out, 1);

  TEST_ASSERT_EQUAL_UINT64(ORC_ERROR_NONE, err);
  TEST_ASSERT_EQUAL_UINT64(ORC_TYPE_F32, out.type_id);
  TEST_ASSERT_EQUAL_UINT64(sizeof(float), out.item_size);
  TEST_ASSERT_EQUAL_UINT64(1, out.n_items);
  float const *result = (float const *)out.items;
  TEST_ASSERT_EQUAL_FLOAT(32.0f, result[0]);
  orc_sdk_handle_free(&in[0]);
  orc_sdk_handle_free(&in[1]);
  orc_sdk_handle_free(&out);
}

static void test_vec_dot_product_multi_row_broadcast(void)
{
  /* Two decks of 3 dvec2 rows each -> 3 scalar dot products, one per row.
     (1,2).(2,1)=4; (3,4).(4,3)=24; (5,6).(6,5)=60. */
  orc_sdk_init(NULL, NULL);
  OrcHandle in[2] = {{0}, {0}}, out = {0};
  in[0].handle = 1;
  in[1].handle = 2;
  out.handle   = 3;
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(_Vec2), &in[0]);
  _Vec2 *v0 = (_Vec2 *)in[0].items;
  orc_sdk_deck_push(v0, ((_Vec2) {1.0, 2.0}), 0);
  orc_sdk_deck_push(v0, ((_Vec2) {3.0, 4.0}), 0);
  orc_sdk_deck_push(v0, ((_Vec2) {5.0, 6.0}), 0);
  in[0].items = v0;
  orc_sdk_oh_update(&in[0]);
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(_Vec2), &in[1]);
  _Vec2 *v1 = (_Vec2 *)in[1].items;
  orc_sdk_deck_push(v1, ((_Vec2) {2.0, 1.0}), 0);
  orc_sdk_deck_push(v1, ((_Vec2) {4.0, 3.0}), 0);
  orc_sdk_deck_push(v1, ((_Vec2) {6.0, 5.0}), 0);
  in[1].items = v1;
  orc_sdk_oh_update(&in[1]);

  OrcError err = VEC_DOT_PRODUCT_INFO.func(0, in, 2, &out, 1);

  TEST_ASSERT_EQUAL_UINT64(ORC_ERROR_NONE, err);
  TEST_ASSERT_EQUAL_UINT64(3, out.n_items);
  TEST_ASSERT_EQUAL_UINT64(sizeof(double), out.item_size);
  double const *result = (double const *)out.items;
  TEST_ASSERT_EQUAL_DOUBLE(4.0, result[0]);
  TEST_ASSERT_EQUAL_DOUBLE(24.0, result[1]);
  TEST_ASSERT_EQUAL_DOUBLE(60.0, result[2]);
  orc_sdk_handle_free(&in[0]);
  orc_sdk_handle_free(&in[1]);
  orc_sdk_handle_free(&out);
}

static void test_vec_dot_product_scalar_arity_one(void)
{
  /* Degenerate case: arity 1 "vectors" (plain scalars). (5) . (6) -> 30. */
  orc_sdk_init(NULL, NULL);
  OrcHandle in[2] = {{0}, {0}}, out = {0};
  in[0].handle = 1;
  in[1].handle = 2;
  out.handle   = 3;
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(double), &in[0]);
  ORC_SDK_DECK_INIT(in[0].items, double, (5.0));
  orc_sdk_oh_update(&in[0]);
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(double), &in[1]);
  ORC_SDK_DECK_INIT(in[1].items, double, (6.0));
  orc_sdk_oh_update(&in[1]);

  OrcError err = VEC_DOT_PRODUCT_INFO.func(0, in, 2, &out, 1);

  TEST_ASSERT_EQUAL_UINT64(ORC_ERROR_NONE, err);
  TEST_ASSERT_EQUAL_UINT64(1, out.n_items);
  double const *result = (double const *)out.items;
  TEST_ASSERT_EQUAL_DOUBLE(30.0, result[0]);
  orc_sdk_handle_free(&in[0]);
  orc_sdk_handle_free(&in[1]);
  orc_sdk_handle_free(&out);
}

/* ============================================================
   vec_dot_product — Error / validation
   ============================================================ */

static void test_vec_dot_product_wrong_n_inputs(void)
{
  /* n_inputs != 2 -> early return, output untouched. */
  orc_sdk_init(NULL, NULL);
  OrcHandle in = {0}, out = {0};
  in.handle  = 1;
  out.handle = 2;
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(double), &in);
  ORC_SDK_DECK_INIT(in.items, double, (1.0));
  orc_sdk_oh_update(&in);

  OrcError err = VEC_DOT_PRODUCT_INFO.func(0, &in, 1, &out, 1);

  TEST_ASSERT_EQUAL_UINT64(ORC_ERROR_INVALID_ARGUMENTS, err);
  TEST_ASSERT_NULL(out.items);
  TEST_ASSERT_NULL(out.free_fn);
  orc_sdk_handle_free(&in);
}

static void test_vec_dot_product_wrong_n_outputs(void)
{
  /* n_outputs != 1 -> early return, output untouched. */
  orc_sdk_init(NULL, NULL);
  OrcHandle in[2] = {{0}, {0}}, out = {0};
  in[0].handle = 1;
  in[1].handle = 2;
  out.handle   = 3;
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(double), &in[0]);
  ORC_SDK_DECK_INIT(in[0].items, double, (1.0));
  orc_sdk_oh_update(&in[0]);
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(double), &in[1]);
  ORC_SDK_DECK_INIT(in[1].items, double, (2.0));
  orc_sdk_oh_update(&in[1]);

  OrcError err = VEC_DOT_PRODUCT_INFO.func(0, in, 2, &out, 2);

  TEST_ASSERT_EQUAL_UINT64(ORC_ERROR_INVALID_ARGUMENTS, err);
  TEST_ASSERT_NULL(out.items);
  TEST_ASSERT_NULL(out.free_fn);
  orc_sdk_handle_free(&in[0]);
  orc_sdk_handle_free(&in[1]);
}

static void test_vec_dot_product_rejects_integer_type(void)
{
  /* i32 is a valid primitive type (unlike make_vec/vec_add/vec_subtract), but dot_product
     only supports float and double -- this is the key type restriction to verify. */
  orc_sdk_init(NULL, NULL);
  OrcHandle in[2] = {{0}, {0}}, out = {0};
  in[0].handle = 1;
  in[1].handle = 2;
  out.handle   = 3;
  orc_sdk_handle_alloc(ORC_TYPE_I32, sizeof(int32_t), &in[0]);
  ORC_SDK_DECK_INIT(in[0].items, int32_t, (1));
  orc_sdk_oh_update(&in[0]);
  orc_sdk_handle_alloc(ORC_TYPE_I32, sizeof(int32_t), &in[1]);
  ORC_SDK_DECK_INIT(in[1].items, int32_t, (2));
  orc_sdk_oh_update(&in[1]);

  OrcError err = VEC_DOT_PRODUCT_INFO.func(0, in, 2, &out, 1);

  TEST_ASSERT_EQUAL_UINT64(ORC_ERROR_TYPE_MISMATCH, err);
  TEST_ASSERT_NULL(out.items);
  TEST_ASSERT_NULL(out.free_fn);
  orc_sdk_handle_free(&in[0]);
  orc_sdk_handle_free(&in[1]);
}

static void test_vec_dot_product_rejects_non_primitive_type(void)
{
  /* Proxies are rejected too, same as any other non-float type. */
  orc_sdk_init(NULL, NULL);
  OrcHandle in[2] = {{0}, {0}}, out = {0};
  in[0].handle = 1;
  in[1].handle = 2;
  out.handle   = 3;
  orc_sdk_handle_alloc(ORC_TYPE_PROXY, sizeof(OrcItemProxy), &in[0]);
  orc_sdk_handle_alloc(ORC_TYPE_PROXY, sizeof(OrcItemProxy), &in[1]);

  OrcError err = VEC_DOT_PRODUCT_INFO.func(0, in, 2, &out, 1);

  TEST_ASSERT_EQUAL_UINT64(ORC_ERROR_TYPE_MISMATCH, err);
  TEST_ASSERT_NULL(out.items);
  TEST_ASSERT_NULL(out.free_fn);
  orc_sdk_handle_free(&in[0]);
  orc_sdk_handle_free(&in[1]);
}

static void test_vec_dot_product_rejects_mismatched_types(void)
{
  /* Both individually valid float types, but must match each other: f32 vs f64. */
  orc_sdk_init(NULL, NULL);
  OrcHandle in[2] = {{0}, {0}}, out = {0};
  in[0].handle = 1;
  in[1].handle = 2;
  out.handle   = 3;
  orc_sdk_handle_alloc(ORC_TYPE_F32, sizeof(float), &in[0]);
  ORC_SDK_DECK_INIT(in[0].items, float, (1.0f));
  orc_sdk_oh_update(&in[0]);
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(double), &in[1]);
  ORC_SDK_DECK_INIT(in[1].items, double, (2.0));
  orc_sdk_oh_update(&in[1]);

  OrcError err = VEC_DOT_PRODUCT_INFO.func(0, in, 2, &out, 1);

  TEST_ASSERT_EQUAL_UINT64(ORC_ERROR_INVALID_ARGUMENTS, err);
  TEST_ASSERT_NULL(out.items);
  TEST_ASSERT_NULL(out.free_fn);
  orc_sdk_handle_free(&in[0]);
  orc_sdk_handle_free(&in[1]);
}

static void test_vec_dot_product_rejects_mismatched_arity(void)
{
  /* dvec2 . dvec3 -- same type_id, but different item_size (arity). */
  orc_sdk_init(NULL, NULL);
  OrcHandle in[2] = {{0}, {0}}, out = {0};
  in[0].handle = 1;
  in[1].handle = 2;
  out.handle   = 3;
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(_Vec2), &in[0]);
  _Vec2 *v0 = (_Vec2 *)in[0].items;
  orc_sdk_deck_push(v0, ((_Vec2) {1.0, 2.0}), 0);
  in[0].items = v0;
  orc_sdk_oh_update(&in[0]);
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(_Vec3), &in[1]);
  _Vec3 *v1 = (_Vec3 *)in[1].items;
  orc_sdk_deck_push(v1, ((_Vec3) {1.0, 2.0, 3.0}), 0);
  in[1].items = v1;
  orc_sdk_oh_update(&in[1]);

  OrcError err = VEC_DOT_PRODUCT_INFO.func(0, in, 2, &out, 1);

  TEST_ASSERT_EQUAL_UINT64(ORC_ERROR_INVALID_ARGUMENTS, err);
  TEST_ASSERT_NULL(out.items);
  TEST_ASSERT_NULL(out.free_fn);
  orc_sdk_handle_free(&in[0]);
  orc_sdk_handle_free(&in[1]);
}

static void test_vec_dot_product_rejects_zero_item_size(void)
{
  /* The first component has item_size == 0 -- invalid handle. */
  orc_sdk_init(NULL, NULL);
  OrcHandle in[2] = {{0}, {0}}, out = {0};
  in[0].handle  = 1;
  in[1].handle  = 2;
  out.handle    = 3;
  in[0].type_id = ORC_TYPE_F64; /* item_size left at 0 -- never allocated. */
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(double), &in[1]);
  ORC_SDK_DECK_INIT(in[1].items, double, (1.0));
  orc_sdk_oh_update(&in[1]);

  OrcError err = VEC_DOT_PRODUCT_INFO.func(0, in, 2, &out, 1);

  TEST_ASSERT_EQUAL_UINT64(ORC_ERROR_INVALID_HANDLE, err);
  TEST_ASSERT_NULL(out.items);
  TEST_ASSERT_NULL(out.free_fn);
  orc_sdk_handle_free(&in[1]);
}

static void test_vec_dot_product_rejects_invalid_aggregate_item_size(void)
{
  /* item_size == 12 is not a multiple of sizeof(double) == 8. Both inputs share this
     item_size so the arity-match check passes and the aggregate check is what fires.
     Constructed via direct field assignment: orc_sdk_handle_alloc itself already rejects
     item_size % scalar_size != 0, so it can't be used to create this handle. */
  orc_sdk_init(NULL, NULL);
  OrcHandle in[2] = {{0}, {0}}, out = {0};
  in[0].handle    = 1;
  in[1].handle    = 2;
  out.handle      = 3;
  in[0].type_id   = ORC_TYPE_F64;
  in[0].item_size = 12;
  in[1].type_id   = ORC_TYPE_F64;
  in[1].item_size = 12;

  OrcError err = VEC_DOT_PRODUCT_INFO.func(0, in, 2, &out, 1);

  TEST_ASSERT_EQUAL_UINT64(ORC_ERROR_INVALID_ARGUMENTS, err);
  TEST_ASSERT_NULL(out.items);
  TEST_ASSERT_NULL(out.free_fn);
}

/* ============================================================
   vec_dot_product — Ownership and lifetime invariants
   ============================================================ */

static void test_vec_dot_product_output_free_fn_set(void)
{
  /* After a successful call, out.free_fn must be set (plugin owns the deck). */
  orc_sdk_init(NULL, NULL);
  OrcHandle in[2] = {{0}, {0}}, out = {0};
  in[0].handle = 1;
  in[1].handle = 2;
  out.handle   = 3;
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(double), &in[0]);
  ORC_SDK_DECK_INIT(in[0].items, double, (1.0));
  orc_sdk_oh_update(&in[0]);
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(double), &in[1]);
  ORC_SDK_DECK_INIT(in[1].items, double, (2.0));
  orc_sdk_oh_update(&in[1]);

  VEC_DOT_PRODUCT_INFO.func(0, in, 2, &out, 1);

  TEST_ASSERT_NOT_NULL(out.free_fn);
  orc_sdk_handle_free(&in[0]);
  orc_sdk_handle_free(&in[1]);
  orc_sdk_handle_free(&out);
}

static void test_vec_dot_product_output_handle_preserved(void)
{
  /* out.handle must be unchanged before and after the call. */
  orc_sdk_init(NULL, NULL);
  OrcHandle in[2] = {{0}, {0}}, out = {0};
  in[0].handle = 1;
  in[1].handle = 2;
  out.handle   = 99;
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(double), &in[0]);
  ORC_SDK_DECK_INIT(in[0].items, double, (1.0));
  orc_sdk_oh_update(&in[0]);
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(double), &in[1]);
  ORC_SDK_DECK_INIT(in[1].items, double, (2.0));
  orc_sdk_oh_update(&in[1]);

  VEC_DOT_PRODUCT_INFO.func(0, in, 2, &out, 1);

  TEST_ASSERT_EQUAL_UINT64(99, out.handle);
  orc_sdk_handle_free(&in[0]);
  orc_sdk_handle_free(&in[1]);
  orc_sdk_handle_free(&out);
}

static void test_vec_dot_product_input_handles_unaffected(void)
{
  /* The input handles' items pointers and n_items must not change. */
  orc_sdk_init(NULL, NULL);
  OrcHandle in[2] = {{0}, {0}}, out = {0};
  in[0].handle = 1;
  in[1].handle = 2;
  out.handle   = 3;
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(double), &in[0]);
  ORC_SDK_DECK_INIT(in[0].items, double, (1.0, 2.0));
  orc_sdk_oh_update(&in[0]);
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(double), &in[1]);
  ORC_SDK_DECK_INIT(in[1].items, double, (3.0, 4.0));
  orc_sdk_oh_update(&in[1]);
  void const *items0_before   = in[0].items;
  uint64_t    n_items0_before = in[0].n_items;
  void const *items1_before   = in[1].items;
  uint64_t    n_items1_before = in[1].n_items;

  VEC_DOT_PRODUCT_INFO.func(0, in, 2, &out, 1);

  TEST_ASSERT_EQUAL_PTR(items0_before, in[0].items);
  TEST_ASSERT_EQUAL_UINT64(n_items0_before, in[0].n_items);
  TEST_ASSERT_EQUAL_PTR(items1_before, in[1].items);
  TEST_ASSERT_EQUAL_UINT64(n_items1_before, in[1].n_items);
  orc_sdk_handle_free(&in[0]);
  orc_sdk_handle_free(&in[1]);
  orc_sdk_handle_free(&out);
}

static void test_vec_dot_product_reuse_output_same_type(void)
{
  /* Calling vec_dot_product twice with the same output type reuses the deck instead of
   * reallocating -- the output is always a single scalar regardless of input arity, so
   * the initial capacity always suffices. */
  orc_sdk_init(NULL, NULL);
  OrcHandle in[2] = {{0}, {0}}, out = {0};
  in[0].handle = 1;
  in[1].handle = 2;
  out.handle   = 3;
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(double), &in[0]);
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(double), &in[1]);

  ORC_SDK_DECK_INIT(in[0].items, double, (2.0));
  orc_sdk_oh_update(&in[0]);
  ORC_SDK_DECK_INIT(in[1].items, double, (3.0));
  orc_sdk_oh_update(&in[1]);
  VEC_DOT_PRODUCT_INFO.func(0, in, 2, &out, 1);
  void const *ptr_after_first = out.items;

  ORC_SDK_DECK_INIT(in[0].items, double, (5.0));
  orc_sdk_oh_update(&in[0]);
  ORC_SDK_DECK_INIT(in[1].items, double, (7.0));
  orc_sdk_oh_update(&in[1]);
  VEC_DOT_PRODUCT_INFO.func(0, in, 2, &out, 1);

  TEST_ASSERT_EQUAL_UINT64(1, out.n_items);
  double const *result = (double const *)out.items;
  TEST_ASSERT_EQUAL_DOUBLE(35.0, result[0]);
  TEST_ASSERT_EQUAL_PTR(ptr_after_first, out.items);

  orc_sdk_handle_free(&in[0]);
  orc_sdk_handle_free(&in[1]);
  orc_sdk_handle_free(&out);
}

static void test_vec_dot_product_output_type_change(void)
{
  /* If out was previously allocated for a different scalar type (or, in this case, from
   * an earlier vec_dot_product call), the next call reallocates it instead of reusing
   * the stale deck. First call is f64, second is f32. */
  orc_sdk_init(NULL, NULL);
  OrcHandle in[2] = {{0}, {0}}, out = {0};
  in[0].handle = 1;
  in[1].handle = 2;
  out.handle   = 3;
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(double), &in[0]);
  ORC_SDK_DECK_INIT(in[0].items, double, (1.0));
  orc_sdk_oh_update(&in[0]);
  orc_sdk_handle_alloc(ORC_TYPE_F64, sizeof(double), &in[1]);
  ORC_SDK_DECK_INIT(in[1].items, double, (2.0));
  orc_sdk_oh_update(&in[1]);
  VEC_DOT_PRODUCT_INFO.func(0, in, 2, &out, 1);
  TEST_ASSERT_EQUAL_UINT64(ORC_TYPE_F64, out.type_id);
  TEST_ASSERT_EQUAL_UINT64(sizeof(double), out.item_size);
  orc_sdk_handle_free(&in[0]);
  orc_sdk_handle_free(&in[1]);

  OrcHandle in2[2] = {{0}, {0}};
  in2[0].handle = 4;
  in2[1].handle = 5;
  orc_sdk_handle_alloc(ORC_TYPE_F32, sizeof(float), &in2[0]);
  ORC_SDK_DECK_INIT(in2[0].items, float, (3.0f));
  orc_sdk_oh_update(&in2[0]);
  orc_sdk_handle_alloc(ORC_TYPE_F32, sizeof(float), &in2[1]);
  ORC_SDK_DECK_INIT(in2[1].items, float, (4.0f));
  orc_sdk_oh_update(&in2[1]);

  VEC_DOT_PRODUCT_INFO.func(0, in2, 2, &out, 1);

  TEST_ASSERT_EQUAL_UINT64(ORC_TYPE_F32, out.type_id);
  TEST_ASSERT_EQUAL_UINT64(sizeof(float), out.item_size);
  float const *result = (float const *)out.items;
  TEST_ASSERT_EQUAL_FLOAT(12.0f, result[0]);
  orc_sdk_handle_free(&in2[0]);
  orc_sdk_handle_free(&in2[1]);
  orc_sdk_handle_free(&out);
}

/* ============================================================ */

int main(void)
{
  UNITY_BEGIN();
  RUN_TEST(test_make_vec_two_scalars_f64);
  RUN_TEST(test_make_vec_three_scalars_f64);
  RUN_TEST(test_make_vec_mixed_arity_components);
  RUN_TEST(test_make_vec_multi_row_broadcast);
  RUN_TEST(test_make_vec_integer_type);
  RUN_TEST(test_make_vec_too_few_inputs);
  RUN_TEST(test_make_vec_wrong_n_outputs);
  RUN_TEST(test_make_vec_rejects_non_primitive_type);
  RUN_TEST(test_make_vec_rejects_mismatched_component_types);
  RUN_TEST(test_make_vec_rejects_zero_item_size);
  RUN_TEST(test_make_vec_rejects_invalid_aggregate_item_size);
  RUN_TEST(test_make_vec_output_free_fn_set);
  RUN_TEST(test_make_vec_output_handle_preserved);
  RUN_TEST(test_make_vec_input_handles_unaffected);
  RUN_TEST(test_make_vec_reuse_output_same_type);
  RUN_TEST(test_make_vec_output_type_change);
  RUN_TEST(test_vec_add_two_dvec2);
  RUN_TEST(test_vec_add_three_dvec3);
  RUN_TEST(test_vec_add_multi_row_broadcast);
  RUN_TEST(test_vec_add_integer_type);
  RUN_TEST(test_vec_add_uint8_type);
  RUN_TEST(test_vec_add_uint32_type);
  RUN_TEST(test_vec_add_too_few_inputs);
  RUN_TEST(test_vec_add_wrong_n_outputs);
  RUN_TEST(test_vec_add_rejects_non_primitive_type);
  RUN_TEST(test_vec_add_rejects_mismatched_types);
  RUN_TEST(test_vec_add_rejects_mismatched_arity);
  RUN_TEST(test_vec_add_rejects_zero_item_size);
  RUN_TEST(test_vec_add_rejects_invalid_aggregate_item_size);
  RUN_TEST(test_vec_add_output_free_fn_set);
  RUN_TEST(test_vec_add_output_handle_preserved);
  RUN_TEST(test_vec_add_input_handles_unaffected);
  RUN_TEST(test_vec_add_reuse_output_same_type);
  RUN_TEST(test_vec_add_output_type_change);
  RUN_TEST(test_vec_subtract_two_dvec2);
  RUN_TEST(test_vec_subtract_multi_row_broadcast);
  RUN_TEST(test_vec_subtract_integer_type);
  RUN_TEST(test_vec_subtract_uint8_type);
  RUN_TEST(test_vec_subtract_uint32_type);
  RUN_TEST(test_vec_subtract_wrong_n_inputs);
  RUN_TEST(test_vec_subtract_wrong_n_outputs);
  RUN_TEST(test_vec_subtract_rejects_non_primitive_type);
  RUN_TEST(test_vec_subtract_rejects_mismatched_types);
  RUN_TEST(test_vec_subtract_rejects_mismatched_arity);
  RUN_TEST(test_vec_subtract_rejects_zero_item_size);
  RUN_TEST(test_vec_subtract_rejects_invalid_aggregate_item_size);
  RUN_TEST(test_vec_subtract_output_free_fn_set);
  RUN_TEST(test_vec_subtract_output_handle_preserved);
  RUN_TEST(test_vec_subtract_input_handles_unaffected);
  RUN_TEST(test_vec_subtract_reuse_output_same_type);
  RUN_TEST(test_vec_subtract_output_type_change);
  RUN_TEST(test_vec_dot_product_two_dvec2_f64);
  RUN_TEST(test_vec_dot_product_two_vec3_f32);
  RUN_TEST(test_vec_dot_product_multi_row_broadcast);
  RUN_TEST(test_vec_dot_product_scalar_arity_one);
  RUN_TEST(test_vec_dot_product_wrong_n_inputs);
  RUN_TEST(test_vec_dot_product_wrong_n_outputs);
  RUN_TEST(test_vec_dot_product_rejects_integer_type);
  RUN_TEST(test_vec_dot_product_rejects_non_primitive_type);
  RUN_TEST(test_vec_dot_product_rejects_mismatched_types);
  RUN_TEST(test_vec_dot_product_rejects_mismatched_arity);
  RUN_TEST(test_vec_dot_product_rejects_zero_item_size);
  RUN_TEST(test_vec_dot_product_rejects_invalid_aggregate_item_size);
  RUN_TEST(test_vec_dot_product_output_free_fn_set);
  RUN_TEST(test_vec_dot_product_output_handle_preserved);
  RUN_TEST(test_vec_dot_product_input_handles_unaffected);
  RUN_TEST(test_vec_dot_product_reuse_output_same_type);
  RUN_TEST(test_vec_dot_product_output_type_change);
  return UNITY_END();
}
