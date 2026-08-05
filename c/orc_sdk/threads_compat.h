#pragma once

// C11 <threads.h> is not available on Apple platforms. Provide a thin
// pthreads shim that exposes only what orc_sdk needs.
#if defined(__APPLE__)
#  include <pthread.h>
typedef pthread_mutex_t mtx_t;
typedef pthread_once_t  once_flag;
#  define mtx_plain       0
#  define ONCE_FLAG_INIT  PTHREAD_ONCE_INIT
static inline int  mtx_init(mtx_t *m, int t) { (void)t; return pthread_mutex_init(m, NULL); }
static inline int  mtx_lock(mtx_t *m)        { return pthread_mutex_lock(m); }
static inline int  mtx_unlock(mtx_t *m)      { return pthread_mutex_unlock(m); }
static inline void call_once(once_flag *f, void (*fn)(void)) { pthread_once(f, fn); }
#else
#  include <threads.h>
#endif
