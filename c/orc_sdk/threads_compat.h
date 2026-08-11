#pragma once

// C11 <threads.h> is not available on Apple platforms, and ThreadSanitizer
// cannot intercept glibc's internal thrd_create→pthread_create call.
// Use a pthreads shim in both cases.
#if !defined(__has_feature)
#define __has_feature(x) 0
#endif
#if defined(__APPLE__) || defined(__SANITIZE_THREAD__) || __has_feature(thread_sanitizer)
#include <pthread.h>
#include <stdint.h>
#include <stdlib.h>
typedef pthread_mutex_t mtx_t;
typedef pthread_once_t  once_flag;
typedef pthread_t       thrd_t;
#define mtx_plain 0
#define thrd_success 0
#define thrd_error 1
#define ONCE_FLAG_INIT PTHREAD_ONCE_INIT
static inline int mtx_init(mtx_t *m, int t)
{
  (void)t;
  return pthread_mutex_init(m, NULL);
}
static inline int mtx_lock(mtx_t *m)
{
  return pthread_mutex_lock(m);
}
static inline int mtx_unlock(mtx_t *m)
{
  return pthread_mutex_unlock(m);
}
static inline void call_once(once_flag *f, void (*fn)(void))
{
  pthread_once(f, fn);
}

// Trampoline to bridge C11 int-returning thread functions to pthreads void*-returning.
typedef struct
{
  int (*fn)(void *);
  void *arg;
} _thrd_tramp_t;
static inline void *_thrd_tramp(void *arg)
{
  _thrd_tramp_t *t = (_thrd_tramp_t *)arg;
  int            r = t->fn(t->arg);
  free(t);
  return (void *)(intptr_t)r;
}
static inline int thrd_create(thrd_t *t, int (*fn)(void *), void *arg)
{
  _thrd_tramp_t *tr = ((_thrd_tramp_t *)malloc(sizeof(_thrd_tramp_t)));
  if (!tr)
    return thrd_error;
  tr->fn  = fn;
  tr->arg = arg;
  return pthread_create(t, NULL, _thrd_tramp, tr) ? thrd_error : thrd_success;
}
static inline int thrd_join(thrd_t t, int *res)
{
  void *rv = NULL;
  if (pthread_join(t, &rv))
    return thrd_error;
  if (res)
    *res = (int)(intptr_t)rv;
  return thrd_success;
}
#else
#include <threads.h>
#endif
