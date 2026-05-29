#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>

#include "status.h"

// ========== Array ==========

typedef struct
{
  size_t count;
  size_t capacity;
} _ArrHeader;

static inline _ArrHeader* _arr_header(void* ptr)
{
  _ArrHeader* h = (_ArrHeader*)ptr;
  if (h)
    return --h;
  return NULL;
}

static inline size_t _arr_capacity(void* ptr)
{
  _ArrHeader* h = _arr_header(ptr);
  if (h)
    return h->capacity;
  return 0;
}

/**
 * Free the array and assign the pointer to NULL.
 */
#define arr_free(ptr) (free(_arr_header((ptr))), (ptr) = NULL)

/**
 * @brief Get the length of the array.
 *
 * @param ptr Pointer to the array.
 * @return size_t Length.
 */
static inline size_t arr_len(void* ptr)
{
  _ArrHeader* h = _arr_header(ptr);
  if (h)
    return h->count;
  return 0;
}

void* _arr_grow(void* ptr, size_t elemsize);

Status _arr_remove_impl(void* ptr, size_t const idx, size_t const elemsize);

/**
 * Remove the element at index idx from the array. This preserves the order of the
 * remaining elements.
 */
#define arr_remove(ptr, idx) _arr_remove_impl((ptr), (idx), sizeof *(ptr))

/**
 * Push a new value to the end of the array.
 */
#define arr_push(ptr, val)                              \
  (((ptr) = _arr_grow(ptr, sizeof *(ptr)))              \
     ? ((ptr)[_arr_header((ptr))->count++] = (val), OK) \
     : ALLOC_FAILED)

/**
 * Get a pointer pointing past the end of the array.
 */
#define arr_end(ptr) (ptr) + arr_len((ptr))

/**
 * Removes the element at the index idx from the array. This is faster than arr_remove,
 * but doesn't preserve the order of the elements.
 */
#define arr_swap_remove(ptr, idx)                                \
  (((ptr) != NULL && (idx) < arr_len(ptr))                       \
     ? ((ptr)[(idx)] = (ptr)[--(_arr_header((ptr))->count)], OK) \
     : OUT_OF_BOUNDS)

void* _arr_grow_capacity(void* ptr, size_t const elemsize, size_t const nelems);

/**
 * Reserve memory for the `size` number of elements in the array.
 */
#define arr_reserve(ptr, size)                                \
  (((ptr) = _arr_grow_capacity((ptr), sizeof *(ptr), (size))) \
     ? OK                                                     \
     : ((size) == 0 ? OK : ALLOC_FAILED))

void* _arr_resize_impl(void* ptr, size_t const elemsize, size_t const count);

/**
 * Resize the array to the given size. If the new size is smaller than the original size,
 * the array is simply truncated. If the size is longer than the original size, the array
 * is extended, and the newly allocated space is zero'd out. Existing elements remain as
 * they were.
 */
#define arr_resize(ptr, size) (ptr) = _arr_resize_impl((ptr), sizeof *(ptr), (size))

/**
 * @brief Clear all elements from the array. This doesn't de-allocate the memory.
 *
 * @param ptr Pointer to the array.
 */
void arr_clear(void* ptr);

void _arr_fill_impl(void*             arr,
                    void const* const elem,
                    size_t const      count,
                    size_t const      elemsize);

#define arr_fill(ptr, val) _arr_fill_impl((ptr), &(val), arr_len((ptr)), sizeof(*ptr))

Status _arr_remove_range_impl(void*        ptr,
                              size_t const start,
                              size_t const stop,
                              size_t const elemsize);

/**
 * Remove a range of elements from the array. The range is from `start` (inclusive) to
 * `stop` (exclusive).
 */
#define arr_remove_range(ptr, start, stop) \
  _arr_remove_range_impl(ptr, start, stop, sizeof *(ptr))

/**
 * Remove the last element of the array and assign it to the `dst` pointer. OK status is
 * returned if the array was not empty. OUT_OF_BOUNDS status is returned if the array was
 * empty.
 */
#define arr_pop(ptr, dst)                                \
  (((ptr) != NULL && arr_len((ptr)) > 0)                 \
     ? (*dst = (ptr)[--(_arr_header((ptr)))->count], OK) \
     : OUT_OF_BOUNDS)

/**
 * @brief Check if the array is empty.
 *
 * @param ptr Pointer to the array.
 * @return bool True if the array is empty.
 */
bool arr_is_empty(void* ptr);

// ========== Queue ==========

typedef struct
{
  size_t back;
  size_t capacity;
  size_t front;
  size_t _padding;
} _QueueHeader;

static inline _QueueHeader* _que_header(void* ptr)
{
  _QueueHeader* h = (_QueueHeader*)ptr;
  if (h)
    return --h;
  return NULL;
}

void* _que_push_grow(void* ptr, size_t const elemsize);

/**
 * @brief Check if the queue is empty.
 *
 * @param ptr Pointer to the queue.
 * @return bool True if the queue is empty.
 */
bool que_is_empty(void* ptr);

size_t _que_pushed_back(void* ptr);

/**
 * Push the new value to the back of the queue. Status is returned indicating whether the
 * operation was successful.
 */
#define que_push(ptr, val)                          \
  (((ptr) = _que_push_grow(ptr, sizeof *(ptr)))     \
     ? ((ptr)[_que_pushed_back((ptr))] = (val), OK) \
     : ALLOC_FAILED)

size_t _que_popped_front(void* ptr);

/**
 * Remove an element from the front of the queue and
 */
#define que_pop(ptr, dst) \
  (que_is_empty((ptr)) ? OUT_OF_BOUNDS : (*(dst) = (ptr)[_que_popped_front((ptr))], OK))

/**
 * @brief Get the length of the queue. This is the remaining items in the queue.
 *
 * @param ptr The pointer to the queue.
 * @return size_t The length.
 */
size_t que_len(void* ptr);

/**
 * Free the queue, and set the pointer to NULL.
 */
#define que_free(ptr) (free(_que_header((ptr))), (ptr) = NULL)

// ========== Hash map ==========

#define HMAP_BUCKET_SIZE 8

typedef struct
{
  size_t    hash[HMAP_BUCKET_SIZE];
  ptrdiff_t index[HMAP_BUCKET_SIZE];
} _HashBucket;

typedef struct
{
  size_t       n_total;
  size_t       n_used;
  size_t       n_removed;
  _HashBucket* buckets;
  _HashBucket* temp_buckets;
  size_t*      slots;
} _HashTableHeader;

static inline _HashTableHeader* _hmap_header(void* ptr)
{
  _HashTableHeader* h = (_HashTableHeader*)ptr;
  if (h)
    return --h;
  return NULL;
}

void* _hmap_grow_size(void* ptr, size_t const kvsize, size_t nelems);

/**
 * Reserve space to store `size` number of entries in the hash table.
 */
#define hmap_reserve(ptr, size) \
  (((ptr) = _hmap_grow_size((ptr), sizeof *(ptr), (size))) ? OK : ALLOC_FAILED)

size_t _hmap_insert_bin_impl(void**       ptr,
                             size_t const kvsize,
                             void*        keyptr,
                             size_t const keysize);

size_t _hmap_insert_str_impl(void** ptr, size_t const kvsize, char const* key);

/**
 * Insert the given key value pair into the hash table. The key should be provided as an
 * lvalue to allow for generic type safe functionality. Use this variant to insert keys
 * that are binary value types.
 */
#define hmap_put(ptr, k, v)                                                 \
  do {                                                                      \
    size_t const idx =                                                      \
      _hmap_insert_bin_impl((void**)&(ptr), sizeof *(ptr), &(k), sizeof k); \
    (ptr)[idx].value = (v);                                                 \
  } while (0)

/**
 * Same as `hmap_put`, but works when the key is a string.
 */
#define hmap_str_put(ptr, k, v)                                                 \
  do {                                                                          \
    size_t const idx = _hmap_insert_str_impl((void**)&(ptr), sizeof *(ptr), k); \
    (ptr)[idx].value = (v);                                                     \
  } while (0)

/**
 * @brief Get the length of the hash map. This is the number of entries in the hash map.
 *
 * @param ptr Pointer to the hash map.
 * @return size_t
 */
size_t hmap_len(void* ptr);

void _hmap_free_impl(void* ptr);

/**
 * Free the hash map, and set the pointer to NULL.
 */
#define hmap_free(ptr) (_hmap_free_impl((ptr)), (ptr) = NULL)

void* _hmap_get_bin_impl(void*        ptr,
                         size_t const kvsize,
                         void*        keyptr,
                         size_t const keysize);

/**
 * Get the pointer to the entry in the hashmap corresponding to the key. If the key
 * doesn't exist in the hash map, NULL is returned. A boid ptr is returned, so it needs to
 * be cast back to the pointer type of the key value pair struct. The key should be passed
 * in as an lvalue. This is required for the API to be generic and typesafe. This only
 * works for keys that are binary value types.
 */
#define hmap_get(ptr, key) _hmap_get_bin_impl((ptr), sizeof *(ptr), &(key), sizeof key)

void* _hmap_get_str_impl(void* ptr, size_t const kvsize, char const* key);

/**
 * Same as `hmap_get`, except this works when the key is a string.
 */
#define hmap_str_get(ptr, key) _hmap_get_str_impl((ptr), sizeof *(ptr), key)

/**
 * Check if the hash map contains the given key.
 */
#define hmap_contains(ptr, key) \
  (_hmap_get_bin_impl((ptr), sizeof *(ptr), &(key), sizeof key) != NULL)

/**
 * Same as `hmap_contains`, except this variant works when the key is a string.
 */
#define hmap_str_contains(ptr, key) \
  (_hmap_get_str_impl((ptr), sizeof *(ptr), key) != NULL)

void _hmap_remove_bin_impl(void*        ptr,
                           size_t const kvsize,
                           void*        keyptr,
                           size_t const keysize);

void _hmap_remove_str_impl(void* ptr, size_t const kvsize, char const* key);

/**
 * Remove the entry from the hashmap corresponding to the given key. If th ekey is not
 * present, the hash map is unaffected.
 */
#define hmap_remove(ptr, key) \
  _hmap_remove_bin_impl((ptr), sizeof *(ptr), &(key), sizeof key)

/**
 * Same as hmap_remove, except this is meant to be used when the key is a string, and not
 * a value type.
 */
#define hmap_str_remove(ptr, key) _hmap_remove_str_impl((ptr), sizeof *(ptr), key)

/**
 * @brief Check if the hash map is empty.
 *
 * @param ptr Pointer to the hashmap.
 * @return bool True if the hash map is empty.
 */
static inline bool hmap_is_empty(void* ptr)
{
  return hmap_len(ptr) == 0;
}

// ========== Hash set - most functionality just reuses hashmaps. ==========

/**
 * Insert the value into the hash set. Must be a value type.
 */
#define hset_put(ptr, val)                                                    \
  do {                                                                        \
    _hmap_insert_bin_impl((void**)&(ptr), sizeof *(ptr), &(val), sizeof val); \
  } while (0)

/**
 * Insert a string into the hash map.
 */
#define hset_str_put(ptr, val)                                 \
  do {                                                         \
    _hmap_insert_str_impl((void**)&(ptr), sizeof *(ptr), val); \
  } while (0)

/**
 * @brief Get the length of the hash map. This is the number of entries.
 *
 * @param ptr
 * @return size_t
 */
static inline size_t hset_len(void* ptr)
{
  return hmap_len(ptr);
}

/**
 * Reserve space for the given number of elements in the hash set.
 */
#define hset_reserve hmap_reserve

/**
 * Check if the hash set contains the given value.
 */
#define hset_contains hmap_contains

/**
 * Check if the hash set contains the given string.
 */
#define hset_str_contains hmap_str_contains

/**
 * Remove the given value from the hash set. If the value is not present in the hash set,
 * it will be unaffected.
 */
#define hset_remove hmap_remove

/**
 * Remove a string from the hash set. If the string is not present in the hash set, it
 * will be unaffected.
 */
#define hset_str_remove hmap_str_remove

/**
 * Free hte hash set, and set the pointer to NULL.
 */
#define hset_free hmap_free

/**
 * @brief Check if the hash set is empty.
 *
 * @param ptr Pointer to the hash set.
 * @return bool True if the hash set is empty.
 */
static inline bool hset_is_empty(void* ptr)
{
  return hset_len(ptr) == 0;
}

// ========== String ==========

#define str_free(ptr) arr_free((ptr))

static inline size_t str_len(void* ptr)
{
  _ArrHeader* h = _arr_header(ptr);
  if (h)
    return h->count == 0 ? 0 : h->count - 1;
  return 0;
}

Status _str_remove_impl(char* const ptr, size_t const idx);

/**
 * @brief Remove a character from a position in a stirng.
 *
 * @param ptr Pointer to the string. This will be overwritten by the macro.
 * @param idx Index to remove the character at.
 */
#define str_remove(ptr, idx) _str_remove_impl((ptr), (idx))

char* _str_push_impl(char*, char);

/**
 * @brief Push a character to the end of the string.
 *
 * @param ptr Pointer to the string. This maybe overwriiten by the macro if an allocation
 * happens.
 * @param ch Character to push.
 */
#define str_push(ptr, ch) (((ptr) = _str_push_impl((ptr), (ch))) ? OK : ALLOC_FAILED)

/**
 * @brief Get the pointer past the end of the string.
 *
 * @param ptr Pointer to the string.
 * @return Pointer just past the last character of the string. Points to the null
 * terminator.
 */
#define str_end(ptr) ((ptr) + str_len((ptr)))

/**
 * @brief Clear the contents of a string.
 *
 * @param ptr The string to clear.
 */
void str_clear(char* ptr);

char* _str_push_str_impl(char*, char const*);

/**
 * @brief Push the `tail` string at the end of the `ptr` string.
 *
 * @param dst The string to append to.
 * @param tail The string to append.
 */
#define str_push_str(dst, tail) \
  (((dst) = _str_push_str_impl((dst), (tail))) ? OK : ALLOC_FAILED)

/**
 * @brief Check if the string is empty.
 *
 * @param ptr The string.
 * @return bool Flag indicating if the string is empty.
 */
static inline bool str_is_empty(void* ptr)
{
  return str_len(ptr) == 0;
}

bool str_eq(char* const a, char* const b);

// ========== String views ==========

typedef struct
{
  char* start;
  char* end;
} StrView;

#define sv_len(sv) \
  (((sv).start == NULL || (sv).end == NULL) ? 0 : (size_t)((sv).end - (sv).start))

static inline bool sv_is_empty(StrView const sv)
{
  return sv.start == sv.end || sv.start == NULL || sv.end == NULL;
}

StrView sv_from_str(char* str);

StrView sv_trim_left(StrView sv);

StrView sv_trim_right(StrView sv);

static inline StrView sv_trim(StrView sv)
{
  return sv_trim_left(sv_trim_right(sv));
}

bool sv_starts_with(StrView const sv, char const* const prefix);

bool sv_ends_with(StrView const sv, char const* const suffix);

bool sv_strip_prefix(StrView* sv, char const* const prefix);

bool sv_strip_suffix(StrView* sv, char const* const suffix);

bool sv_contains_str(StrView const sv, char const* const needle);

StrView sv_split_at_delim(StrView* sv, char const delim);

#define sv_split_line(sv) sv_split_at_delim((sv), '\n')

char* sv_find(StrView sv, char const c);

static inline bool sv_contains_char(StrView sv, char const c)
{
  return sv_find(sv, c) != NULL;
}

char* sv_rfind(StrView sv, char const c);

StrView sv_slice(StrView sv, size_t const start, size_t const end);

bool sv_eq(StrView const a, StrView const b);

// ========== Deck ==========

typedef struct
{
  uint8_t  depth;
  uint64_t pos;
} Mark;

typedef struct
{
  Mark*     marks;
  uint64_t* stride_offset;
  uint64_t* strides;
  size_t*   pegs;
  size_t    count;
  size_t    capacity;
} _DeckHeader;

static inline _DeckHeader* _deck_header(void* ptr)
{
  _DeckHeader* h = (_DeckHeader*)ptr;
  if (h)
    return --h;
  return NULL;
}

void _deck_free_impl(void* ptr);

#define deck_free(ptr) (_deck_free_impl((ptr)), (ptr) = NULL)

uint8_t deck_max_depth(void* deck);

static inline size_t deck_len(void* deck)
{
  _DeckHeader* h = _deck_header(deck);
  if (h)
    return h->count;
  return 0;
}

static inline bool deck_is_empty(void* deck)
{
  return deck_len(deck) == 0;
}

void* _deck_push_impl(void* ptr, void* item, size_t const itemsize, uint8_t const depth);

#define deck_push(ptr, item, depth) \
  (((ptr) = _deck_push_impl((ptr), &(item), sizeof(*ptr), (depth))) ? OK : ALLOC_FAILED)

void deck_clear(void* ptr);

void* _deck_grow_capacity(void* ptr, size_t const itemsize, size_t const n);

#define deck_reserve(ptr, size)                                \
  (((ptr) = _deck_grow_capacity((ptr), sizeof *(ptr), (size))) \
     ? OK                                                      \
     : ((size) == 0 ? OK : ALLOC_FAILED))

void deck_flatten(void* ptr);

void deck_graft(void* ptr);

void deck_simplify(void* ptr);
