/*
 * cli_client — A simple C client that talks to the orc server.
 *
 * Usage: cli_client [host] [port]
 *
 * Demonstrates:
 *   1. Starting a session
 *   2. Creating f64 deck constants via raw ABI serialization
 *   3. Calling the "add" function
 *   4. Downloading and deserializing the result
 *   5. Closing the session
 */

#include <orc_sdk/orc_sdk.h>

/* ==================== Platform socket abstraction ==================== */

#ifdef _WIN32
#define WIN32_LEAN_AND_MEAN
#include <winsock2.h>
#include <ws2tcpip.h>
#pragma comment(lib, "ws2_32.lib")
typedef SOCKET sock_t;
#define SOCK_INVALID INVALID_SOCKET
#define sock_close closesocket
static int sock_init(void)
{
  WSADATA wsa;
  return WSAStartup(MAKEWORD(2, 2), &wsa);
}
static void sock_cleanup(void)
{
  WSACleanup();
}
#else
#include <arpa/inet.h>
#include <netdb.h>
#include <sys/socket.h>
#include <unistd.h>
typedef int sock_t;
#define SOCK_INVALID (-1)
#define sock_close close
static int  sock_init(void)
{
  return 0;
}
static void sock_cleanup(void) {}
#endif

/* ==================== Growable byte buffer ==================== */

typedef struct
{
  char  *data;
  size_t len;
  size_t cap;
} Buf;

static void buf_init(Buf *b)
{
  b->data = NULL;
  b->len  = 0;
  b->cap  = 0;
}

static void buf_free(Buf *b)
{
  free(b->data);
  buf_init(b);
}

static int buf_grow(Buf *b, size_t extra)
{
  size_t need = b->len + extra;
  if (need <= b->cap)
    return 0;
  size_t newcap = b->cap ? b->cap : 256;
  while (newcap < need)
    newcap *= 2;
  char *p = (char *)realloc(b->data, newcap);
  if (!p)
    return -1;
  b->data = p;
  b->cap  = newcap;
  return 0;
}

static int buf_append(Buf *b, void const *src, size_t n)
{
  if (buf_grow(b, n) != 0)
    return -1;
  memcpy(b->data + b->len, src, n);
  b->len += n;
  return 0;
}

static int buf_append_str(Buf *b, char const *s)
{
  return buf_append(b, s, strlen(s));
}

/* ==================== Minimal HTTP client ==================== */

typedef struct
{
  int    status;
  char  *body;
  size_t body_len;
} HttpResponse;

static void http_response_free(HttpResponse *r)
{
  free(r->body);
}

static sock_t tcp_connect(char const *host, uint16_t port)
{
  struct addrinfo hints, *res;
  memset(&hints, 0, sizeof(hints));
  hints.ai_family   = AF_INET;
  hints.ai_socktype = SOCK_STREAM;
  char port_str[8];
  snprintf(port_str, sizeof(port_str), "%u", (unsigned)port);
  if (getaddrinfo(host, port_str, &hints, &res) != 0)
    return SOCK_INVALID;
  sock_t s = socket(res->ai_family, res->ai_socktype, res->ai_protocol);
  if (s == SOCK_INVALID) {
    freeaddrinfo(res);
    return SOCK_INVALID;
  }
  if (connect(s, res->ai_addr, (int)res->ai_addrlen) != 0) {
    sock_close(s);
    freeaddrinfo(res);
    return SOCK_INVALID;
  }
  freeaddrinfo(res);
  return s;
}

static int send_all(sock_t s, char const *data, size_t len)
{
  while (len > 0) {
    int n = send(s, data, (int)len, 0);
    if (n <= 0)
      return -1;
    data += n;
    len -= (size_t)n;
  }
  return 0;
}

/* Receive the full HTTP response. Handles Content-Length. */
static int recv_response(sock_t s, HttpResponse *out)
{
  memset(out, 0, sizeof(*out));
  Buf raw;
  buf_init(&raw);
  /* Read until we have the full header (terminated by \r\n\r\n). */
  for (;;) {
    if (buf_grow(&raw, 1024) != 0) {
      buf_free(&raw);
      return -1;
    }
    int n = recv(s, raw.data + raw.len, 1024, 0);
    if (n <= 0) {
      if (n == 0)
        break;
      buf_free(&raw);
      return -1;
    }
    raw.len += (size_t)n;
    /* Check if header is complete. */
    if (raw.len >= 4) {
      char *hdr_end = NULL;
      for (size_t i = 0; i <= raw.len - 4; i++) {
        if (memcmp(raw.data + i, "\r\n\r\n", 4) == 0) {
          hdr_end = raw.data + i;
          break;
        }
      }
      if (hdr_end)
        break;
    }
  }
  /* Parse status line. */
  char *hdr_end = NULL;
  for (size_t i = 0; i <= raw.len - 4; i++) {
    if (memcmp(raw.data + i, "\r\n\r\n", 4) == 0) {
      hdr_end = raw.data + i;
      break;
    }
  }
  if (!hdr_end) {
    buf_free(&raw);
    return -1;
  }
  /* Null-terminate header for string ops. */
  *hdr_end           = '\0';
  size_t header_len  = (size_t)(hdr_end - raw.data);
  size_t body_offset = header_len + 4;
  size_t body_so_far = raw.len - body_offset;
  /* Parse "HTTP/1.x NNN" */
  if (sscanf(raw.data, "HTTP/%*d.%*d %d", &out->status) != 1) {
    buf_free(&raw);
    return -1;
  }
  /* Find Content-Length. */
  size_t      content_length = 0;
  char const *cl             = strstr(raw.data, "Content-Length:");
  if (!cl)
    cl = strstr(raw.data, "content-length:");
  if (cl) {
    cl += strlen("Content-Length:");
    while (*cl == ' ')
      cl++;
    content_length = (size_t)atol(cl);
  }
  /* Read remaining body bytes. */
  while (body_so_far < content_length) {
    size_t need = content_length - body_so_far;
    if (buf_grow(&raw, need) != 0) {
      buf_free(&raw);
      return -1;
    }
    int n = recv(s, raw.data + raw.len, (int)need, 0);
    if (n <= 0) {
      buf_free(&raw);
      return -1;
    }
    raw.len += (size_t)n;
    body_so_far += (size_t)n;
  }
  /* Copy body out. */
  out->body_len = content_length;
  out->body     = (char *)malloc(content_length + 1);
  if (!out->body) {
    buf_free(&raw);
    return -1;
  }
  memcpy(out->body, raw.data + body_offset, content_length);
  out->body[content_length] = '\0';
  buf_free(&raw);
  return 0;
}

static int http_request(char const   *host,
                        uint16_t      port,
                        char const   *method,
                        char const   *path,
                        char const   *content_type,
                        void const   *body,
                        size_t        body_len,
                        HttpResponse *out)
{
  sock_t s = tcp_connect(host, port);
  if (s == SOCK_INVALID)
    return -1;
  /* Build request line + headers. */
  Buf req;
  buf_init(&req);
  buf_append_str(&req, method);
  buf_append_str(&req, " ");
  buf_append_str(&req, path);
  buf_append_str(&req, " HTTP/1.1\r\nHost: ");
  buf_append_str(&req, host);
  buf_append_str(&req, "\r\nConnection: close\r\n");
  if (content_type) {
    buf_append_str(&req, "Content-Type: ");
    buf_append_str(&req, content_type);
    buf_append_str(&req, "\r\n");
  }
  char cl_hdr[64];
  snprintf(cl_hdr, sizeof(cl_hdr), "Content-Length: %zu\r\n", body_len);
  buf_append_str(&req, cl_hdr);
  buf_append_str(&req, "\r\n");
  if (send_all(s, req.data, req.len) != 0) {
    buf_free(&req);
    sock_close(s);
    return -1;
  }
  buf_free(&req);
  if (body_len > 0 && body) {
    if (send_all(s, (char const *)body, body_len) != 0) {
      sock_close(s);
      return -1;
    }
  }
  int rc = recv_response(s, out);
  sock_close(s);
  return rc;
}

static int http_get_json(char const   *host,
                         uint16_t      port,
                         char const   *path,
                         HttpResponse *out)
{
  return http_request(host, port, "GET", path, NULL, NULL, 0, out);
}

static int http_post_json(char const   *host,
                          uint16_t      port,
                          char const   *path,
                          char const   *json_body,
                          HttpResponse *out)
{
  return http_request(
    host, port, "POST", path, "application/json", json_body, strlen(json_body), out);
}

static int http_post_bytes(char const   *host,
                           uint16_t      port,
                           char const   *path,
                           void const   *data,
                           size_t        data_len,
                           HttpResponse *out)
{
  return http_request(
    host, port, "POST", path, "application/octet-stream", data, data_len, out);
}

/* ==================== Simple JSON value extraction ==================== */

/* Find "key": <number> in a JSON string and return the number. */
static int json_get_u64(char const *json, char const *key, uint64_t *out)
{
  char pattern[128];
  snprintf(pattern, sizeof(pattern), "\"%s\"", key);
  char const *p = strstr(json, pattern);
  if (!p)
    return -1;
  p += strlen(pattern);
  while (*p == ' ' || *p == ':')
    p++;
  char  *end;
  double val = strtod(p, &end);
  if (end == p)
    return -1;
  *out = (uint64_t)val;
  return 0;
}

/* Find "key": [n1, n2, ...] and fill array. Returns count. */
static int json_get_u64_arr(char const *json, char const *key, uint64_t *out, int max)
{
  char pattern[128];
  snprintf(pattern, sizeof(pattern), "\"%s\"", key);
  char const *p = strstr(json, pattern);
  if (!p)
    return -1;
  p += strlen(pattern);
  while (*p && *p != '[')
    p++;
  if (*p != '[')
    return -1;
  p++;
  int count = 0;
  while (*p && *p != ']' && count < max) {
    while (*p == ' ' || *p == ',')
      p++;
    if (*p == ']')
      break;
    char  *end;
    double val = strtod(p, &end);
    if (end == p)
      break;
    out[count++] = (uint64_t)val;
    p            = end;
  }
  return count;
}

/* ==================== Handle serialization via orc_sdk ==================== */

/* serial_write callback: ctx is a Buf* pointer. */
static OrcError client_serial_write(uint64_t const ctx,
                                    void const    *data,
                                    uint64_t const len)
{
  Buf *b = (Buf *)(uintptr_t)ctx;
  if (buf_append(b, data, (size_t)len) != 0)
    return ORC_ERROR_ALLOC_FAILED;
  return ORC_ERROR_NONE;
}

static void sdk_init_once(void)
{
  static bool done = false;
  if (done)
    return;
  done = true;
  OrcHost host;
  memset(&host, 0, sizeof(host));
  host.abi_version            = ORC_ABI_VERSION;
  host.callbacks.serial_write = client_serial_write;
  orc_sdk_init(&host, NULL);
}

static int serialize_handle(OrcHandle const *handle, Buf *out)
{
  uint64_t ctx = (uint64_t)(uintptr_t)out;
  OrcError err = orc_sdk_serialize_handle_header(ctx, handle);
  if (err != ORC_ERROR_NONE)
    return -1;
  if (handle->n_items > 0 && handle->items != NULL) {
    ORC_SDK_REQUIRE(handle->item_size > 0);
    err =
      orc_sdk_host_serial_write(ctx, handle->items, handle->n_items * handle->item_size);
    if (err != ORC_ERROR_NONE)
      return -1;
  }
  return 0;
}

static int deserialize_handle(void const *data, size_t data_len, OrcHandle *out)
{
  OrcStrView src;
  src.start      = (char *)data;
  src.end        = (char *)data + data_len;
  OrcMark *marks = NULL;
  OrcError err   = orc_sdk_deserialize_handle_header(0, &src, out, &marks);
  if (err != ORC_ERROR_NONE) {
    fprintf(stderr, "Deserialize header failed: 0x%x\n", err);
    return -1;
  }
  ORC_SDK_REQUIRE(out->item_size > 0);
  size_t items_bytes = (size_t)(out->n_items * out->item_size);
  void  *items       = NULL;
  if (items_bytes > 0) {
    items = malloc(items_bytes);
    if (!items) {
      orc_sdk_arr_free(marks);
      return -1;
    }
    err = orc_sdk_sv_read_bytes(&src, items, items_bytes);
    if (err != ORC_ERROR_NONE) {
      free(items);
      orc_sdk_arr_free(marks);
      return -1;
    }
  }
  out->items = items;
  out->marks = marks;
  if (!orc_sv_is_empty(src)) {
    fprintf(stderr, "Trailing bytes after deserialization\n");
    free(items);
    orc_sdk_arr_free(marks);
    return -1;
  }
  return 0;
}

static void free_deserialized_handle(OrcHandle *h)
{
  free((void *)h->items);
  OrcMark *marks = (OrcMark *)h->marks;
  orc_sdk_arr_free(marks);
  memset(h, 0, sizeof(*h));
}

/* ==================== Main ==================== */

static void error_abort(char const *msg)
{
  fprintf(stderr, "ERROR: %s\n", msg);
  exit(1);
}

int main(int argc, char **argv)
{
  char const *host = "127.0.0.1";
  uint16_t    port = 8222;
  if (argc > 1)
    host = argv[1];
  if (argc > 2)
    port = (uint16_t)atoi(argv[2]);

  sdk_init_once();
  if (sock_init() != 0)
    error_abort("Failed to initialize sockets");

  printf("Connecting to %s:%u...\n", host, port);
  HttpResponse resp;

  /* 1. List functions. */
  if (http_get_json(host, port, "/functions", &resp) != 0)
    error_abort("GET /functions failed");
  printf("Functions: %s\n", resp.body);
  http_response_free(&resp);

  /* 2. Start session. */
  if (http_post_json(host, port, "/session/start", "{}", &resp) != 0)
    error_abort("POST /session/start failed");
  if (resp.status != 200) {
    fprintf(stderr, "Start session failed: %d %s\n", resp.status, resp.body);
    http_response_free(&resp);
    error_abort("Session start failed");
  }
  uint64_t session_id;
  if (json_get_u64(resp.body, "session_id", &session_id) != 0)
    error_abort("Failed to parse session_id");
  printf("Session started: %llu\n", (unsigned long long)session_id);
  http_response_free(&resp);

  /* 3. Create constant: [1.0, 2.0, 3.0] */
  double    a_values[] = {1.0, 2.0, 3.0};
  OrcHandle a_handle;
  memset(&a_handle, 0, sizeof(a_handle));
  a_handle.type_id   = ORC_TYPE_F64;
  a_handle.n_items   = 3;
  a_handle.item_size = sizeof(double);
  a_handle.items     = a_values;
  Buf a_buf;
  buf_init(&a_buf);
  if (serialize_handle(&a_handle, &a_buf) != 0)
    error_abort("Failed to serialize deck A");
  char path[256];
  snprintf(
    path, sizeof(path), "/constant?session_id=%llu", (unsigned long long)session_id);
  if (http_post_bytes(host, port, path, a_buf.data, a_buf.len, &resp) != 0)
    error_abort("POST /constant (A) failed");
  buf_free(&a_buf);
  if (resp.status != 200) {
    fprintf(stderr, "Constant A failed: %d %s\n", resp.status, resp.body);
    http_response_free(&resp);
    error_abort("Constant A failed");
  }
  uint64_t a_id;
  if (json_get_u64(resp.body, "handle_id", &a_id) != 0)
    error_abort("Failed to parse handle_id for A");
  printf("Constant A (handle %llu): [1.0, 2.0, 3.0]\n", (unsigned long long)a_id);
  http_response_free(&resp);

  /* 4. Create constant: [10.0, 20.0, 30.0] */
  double    b_values[] = {10.0, 20.0, 30.0};
  OrcHandle b_handle;
  memset(&b_handle, 0, sizeof(b_handle));
  b_handle.type_id   = ORC_TYPE_F64;
  b_handle.n_items   = 3;
  b_handle.item_size = sizeof(double);
  b_handle.items     = b_values;
  Buf b_buf;
  buf_init(&b_buf);
  if (serialize_handle(&b_handle, &b_buf) != 0)
    error_abort("Failed to serialize deck B");
  if (http_post_bytes(host, port, path, b_buf.data, b_buf.len, &resp) != 0)
    error_abort("POST /constant (B) failed");
  buf_free(&b_buf);
  if (resp.status != 200) {
    fprintf(stderr, "Constant B failed: %d %s\n", resp.status, resp.body);
    http_response_free(&resp);
    error_abort("Constant B failed");
  }
  uint64_t b_id;
  if (json_get_u64(resp.body, "handle_id", &b_id) != 0)
    error_abort("Failed to parse handle_id for B");
  printf("Constant B (handle %llu): [10.0, 20.0, 30.0]\n", (unsigned long long)b_id);
  http_response_free(&resp);

  /* 5. Call add(A, B). */
  char call_body[256];
  snprintf(call_body,
           sizeof(call_body),
           "{\"session_id\": %llu, \"function\": \"add\", \"inputs\": [%llu, %llu]}",
           (unsigned long long)session_id,
           (unsigned long long)a_id,
           (unsigned long long)b_id);
  if (http_post_json(host, port, "/call", call_body, &resp) != 0)
    error_abort("POST /call failed");
  if (resp.status != 200) {
    fprintf(stderr, "Call failed: %d %s\n", resp.status, resp.body);
    http_response_free(&resp);
    error_abort("Call failed");
  }
  uint64_t output_ids[16];
  int      n_outputs = json_get_u64_arr(resp.body, "output_ids", output_ids, 16);
  if (n_outputs < 1)
    error_abort("No output_ids in call response");
  printf("add() returned handle %llu\n", (unsigned long long)output_ids[0]);
  http_response_free(&resp);

  /* 6. Download result. */
  snprintf(path,
           sizeof(path),
           "/download?session_id=%llu&handle_id=%llu",
           (unsigned long long)session_id,
           (unsigned long long)output_ids[0]);
  if (http_post_bytes(host, port, path, NULL, 0, &resp) != 0)
    error_abort("POST /download failed");
  if (resp.status != 200) {
    fprintf(stderr, "Download failed: %d %s\n", resp.status, resp.body);
    http_response_free(&resp);
    error_abort("Download failed");
  }
  OrcHandle result;
  memset(&result, 0, sizeof(result));
  if (deserialize_handle(resp.body, resp.body_len, &result) != 0) {
    http_response_free(&resp);
    error_abort("Failed to deserialize result");
  }
  http_response_free(&resp);
  printf("Result (type_id=0x%llx, n_items=%llu): [",
         (unsigned long long)result.type_id,
         (unsigned long long)result.n_items);
  if (result.type_id == ORC_TYPE_F64) {
    double const *vals = (double const *)result.items;
    for (uint64_t i = 0; i < result.n_items; i++) {
      if (i > 0)
        printf(", ");
      printf("%.1f", vals[i]);
    }
  }
  printf("]\n");
  free_deserialized_handle(&result);

  /* 7. Close session. */
  char close_body[128];
  snprintf(close_body,
           sizeof(close_body),
           "{\"session_id\": %llu}",
           (unsigned long long)session_id);
  if (http_post_json(host, port, "/session/close", close_body, &resp) != 0)
    error_abort("POST /session/close failed");
  printf("Session closed (status %d).\n", resp.status);
  http_response_free(&resp);

  sock_cleanup();
  printf("Done.\n");
  return 0;
}
