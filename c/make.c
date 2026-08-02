#include <stdbool.h>
#include <stdio.h>
#include <string.h>

#define STB_C_LEXER_IMPLEMENTATION

#include "third_party/stb_c_lexer.h"

#define NOB_IMPLEMENTATION

#include "third_party/nob.h"

#define SDK_SRC_DIR "orc_sdk/"
#define DECK_OPS_PLUGIN_SRC_DIR "deck_ops_plugin/"

#if defined(_WIN32) || defined(_WIN64)
#define DECK_OPS_PLUGIN_FILENAME "deck_ops_plugin.dll"
#elif defined(__APPLE__)
#define DECK_OPS_PLUGIN_FILENAME "deck_ops_plugin.dylib"
#else
#define DECK_OPS_PLUGIN_FILENAME "deck_ops_plugin.so"
#endif

static void cc_append_flags(Nob_Cmd *cmd, bool is_release)
{
  nob_cc_flags(cmd);
  nob_cmd_append(cmd, "-std=c99", "-pedantic");
  if (is_release)
    nob_cmd_append(cmd, "-O3");
  nob_cmd_append(cmd, "-I.");  // Include the current directory.
  nob_cmd_append(cmd,
                 "-Werror",
                 "-Wformat=2",
                 "-Wconversion",
                 "-Wsign-conversion",
                 "-Wcast-align",
                 "-Wpointer-arith",
                 "-Winit-self",
                 "-Wshadow",
                 "-Wstrict-overflow=5");
}

static void discover_tests_from_src(char const         *src,
                                    size_t const        size,
                                    Nob_String_Builder *dst,
                                    size_t             *counter,
                                    char *const         test_filter)
{
  stb_lexer lex;
  char      string_storage[4096];
  stb_c_lexer_init(&lex, src, src + size, string_storage, sizeof(string_storage));
  while (stb_c_lexer_get_token(&lex)) {
    if (lex.token == CLEX_parse_error) {
      nob_log(NOB_ERROR,
              "Unable to discover tests. String storage likey needs to be larger.");
      return;
    }
    // Look for "void" keyword
    if (lex.token == CLEX_id && strcmp(lex.string, "void") == 0) {
      // Get next token (should be function name)
      if (stb_c_lexer_get_token(&lex) && lex.token == CLEX_id) {
        // Check if function name starts with "t_"
        if (strncmp(lex.string, "test_", 5) == 0) {
          char func_name[256];
          strncpy(func_name, lex.string, sizeof(func_name) - 1);
          func_name[sizeof(func_name) - 1] = '\0';
          // Get next token - must be '(' for function
          if (stb_c_lexer_get_token(&lex) && lex.token == '(') {
            // Skip parameter list - look for closing ')'
            int paren_depth = 1;
            while (paren_depth > 0 && stb_c_lexer_get_token(&lex)) {
              if (lex.token == '(')
                paren_depth++;
              else if (lex.token == ')')
                paren_depth--;
            }
            // Get next token - must be '{' for function definition
            if (stb_c_lexer_get_token(&lex) && lex.token == '{') {
              // Confirmed: void test_something(...) {
              if (test_filter == NULL || strstr(func_name, test_filter) != NULL) {
                nob_log(NOB_INFO, "\t\tFound %s", func_name);
                nob_sb_append_cstr(dst, func_name);
                nob_sb_append_cstr(dst, "\n");
                ++(*counter);
              }
            }
          }
        }
      }
    }
  }
}

static bool list_src_files(Nob_File_Paths *dst, char const *dir)
{
  dst->count = 0;
  // We read all files and then filter out the ones we don't want to compile.
  if (!nob_read_entire_dir(dir, dst)) {
    nob_log(NOB_ERROR, "Unable to read test files from the test directory.");
    return false;
  }
  for (size_t i = 0; i < dst->count; ++i) {
    Nob_String_View sv = nob_sv_from_cstr(dst->items[i]);
    if (nob_sv_end_with(sv, ".c") || nob_sv_end_with(sv, ".h")) {
      dst->items[i] = nob_temp_sprintf("%s%s", dir, dst->items[i]);
    }
    else {
      nob_da_remove_unordered(dst, i--);
    }
  }
  return true;
}

static bool build_and_run_tests(Nob_File_Paths const files,
                                char *const          test_filter,
                                bool                 is_release,
                                char const          *test_runner_src,
                                char const          *test_runner_bin)
{
  Nob_String_Builder contents    = {0};
  Nob_String_Builder sbtestnames = {0};
  Nob_String_Builder sbcode      = {0};
  Nob_Cmd            cmd         = {0};
  size_t             counter     = 0;
  bool               success     = true;
  // Scan all test files and discover tests.
  {
    char const **begin = files.items;
    char const **end   = begin + files.count;
    while (begin != end) {
      char const *fpath = *(begin++);
      if (!nob_sv_end_with(nob_sv_from_cstr(fpath), ".c")) {
        nob_log(NOB_INFO, "Skipping test discovery in %s...", fpath);
        continue;
      }
      nob_log(NOB_INFO, "Looking for tests in %s...", fpath);
      // Get the file path.
      // Read the file.
      contents.count = 0;
      if (!nob_read_entire_file(fpath, &contents)) {
        nob_log(NOB_ERROR, "Unable to read the contents of the file: %s", fpath);
        success = false;
        goto cleanup;
      }
      nob_sb_append_null(&contents);
      discover_tests_from_src(
        contents.items, contents.count, &sbtestnames, &counter, test_filter);
    }
    nob_log(NOB_INFO, "Found %zu tests in total.", counter);
  }
  nob_sb_append_cstr(&sbcode,
                     "// IMPORTANT: This file is autogenerated before building.\n\n"
                     "#include <stdio.h>\n\n");
  {  // Append declarations.
    Nob_String_View testnames = nob_sb_to_sv(sbtestnames);
    while (testnames.count) {
      Nob_String_View name = nob_sv_chop_by_delim(&testnames, '\n');
      nob_sb_append_cstr(&sbcode, "void ");
      nob_sb_append_buf(&sbcode, name.data, name.count);
      nob_sb_append_cstr(&sbcode, "(void);\n");
    }
  }
  {  // Append main function that calls test functions.
    nob_sb_append_cstr(&sbcode, "\nint main(void) {\n");
    Nob_String_View testnames = nob_sb_to_sv(sbtestnames);
    while (testnames.count) {
      Nob_String_View name = nob_sv_chop_by_delim(&testnames, '\n');
      nob_sb_append_cstr(&sbcode, "    printf(\"\\t\\t");
      nob_sb_append_buf(&sbcode, name.data, name.count);
      nob_sb_append_cstr(&sbcode, "    ...\");\n");
      nob_sb_append_cstr(&sbcode, "    fflush(stdout);\n");
      nob_sb_append_cstr(&sbcode, "    ");
      nob_sb_append_buf(&sbcode, name.data, name.count);
      nob_sb_append_cstr(&sbcode, "();\n");
      nob_sb_append_cstr(&sbcode, "    printf(\"ok\\n\");\n");
    }
    nob_sb_append_cstr(&sbcode,
                       "    printf(\"\\nAll tests passed!\\n\\n\");\n"
                       "    return 0;\n}\n");
  }
  if (!nob_write_entire_file(test_runner_src, sbcode.items, sbcode.count)) {
    nob_log(NOB_ERROR, "Unable to write out the test runner code to file.");
    success = false;
    goto cleanup;
  }
  nob_log(NOB_INFO, "Generated the test runner code.");
build:
  if (!nob_needs_rebuild(test_runner_bin, files.items, files.count) &&
      !nob_needs_rebuild1(test_runner_bin, test_runner_src)) {
    nob_log(NOB_INFO, "Test runner is already up to date. Skipping rebuild.");
    goto run;
  }
  // Now build the test runner.
  cmd.count = 0;
  nob_cc(&cmd);
  cc_append_flags(&cmd, is_release);
  nob_cc_output(&cmd, test_runner_bin);
  for (size_t i = 0; i < files.count; ++i) {
    Nob_String_View sv = nob_sv_from_cstr(files.items[i]);
    if (nob_sv_end_with(sv, ".c")) {
      nob_cc_inputs(&cmd, files.items[i]);
    }
  }
  nob_cc_inputs(&cmd, test_runner_src);
  nob_cmd_append(&cmd, "-lm");  // Link math.
  if (!nob_cmd_run_sync(cmd)) {
    nob_log(NOB_ERROR, "Unable to build the test runner");
    success = false;
    goto cleanup;
  }
  nob_log(NOB_INFO, "Built the test runner. Now running tests...");
run:
  cmd.count = 0;
  nob_cmd_append(&cmd, test_runner_bin);
  if (!nob_cmd_run_sync(cmd)) {
    nob_log(NOB_ERROR, "Tests failed");
    success = false;
    goto cleanup;
  }
cleanup:
  nob_cmd_free(cmd);
  nob_sb_free(contents);
  nob_sb_free(sbtestnames);
  nob_sb_free(sbcode);
  return success;
}

int main(int argc, char **argv)
{
  NOB_GO_REBUILD_URSELF(argc, argv);
  if (argc > 3) {  // Too many arguments.
    return -1;
  }
  bool  is_release  = false;
  char *test_filter = NULL;
  for (int i = 1; i < argc; ++i) {
    if (strcmp(argv[i], "--release") == 0) {
      is_release = true;
    }
    else {
      test_filter = argv[i];
    }
  }
  char const *build_dir       = is_release ? "../build/release/" : "../build/debug/";
  char const *test_runner_src = nob_temp_sprintf("%s_test_runner.c", build_dir);
  char const *test_runner_bin = nob_temp_sprintf("%stest_runner", build_dir);
  char const *deck_ops_out = nob_temp_sprintf("%s" DECK_OPS_PLUGIN_FILENAME, build_dir);
  int         ret          = 0;
  if (!nob_mkdir_if_not_exists("build/") || !nob_mkdir_if_not_exists(build_dir)) {
    ret = 1;
    goto cleanup;
  }
  Nob_File_Paths srcfiles = {0};
  {  // Build and run the unit tests of the SDK. These tests must pass before we build
     // anything else.
    if (!list_src_files(&srcfiles, SDK_SRC_DIR)) {
      ret = 1;
      goto cleanup;
    }
    if (!build_and_run_tests(
          srcfiles, test_filter, is_release, test_runner_src, test_runner_bin)) {
      ret = 1;
      goto cleanup;
    }
  }
  {  // Build deck_ops_plugin.
     // srcfiles already contains the SDK source files. Append them to the command as
     // inputs.
    Nob_Cmd cmd = {0};
    nob_cc(&cmd);
    cc_append_flags(&cmd, is_release);
    nob_cmd_append(&cmd, "-shared", "-fPIC");
    nob_cc_output(&cmd, deck_ops_out);
    for (size_t i = 0; i < srcfiles.count; ++i) {
      if (nob_sv_end_with(nob_sv_from_cstr(srcfiles.items[i]), ".c")) {
        nob_cc_inputs(&cmd, srcfiles.items[i]);
      }
    }
    // Check if the SDK source files have been modified.
    bool const rebuild = nob_needs_rebuild(deck_ops_out, srcfiles.items, srcfiles.count);
    if (!list_src_files(&srcfiles, DECK_OPS_PLUGIN_SRC_DIR)) {
      ret = 1;
      goto cleanup;
    }
    if (rebuild || nob_needs_rebuild(deck_ops_out, srcfiles.items, srcfiles.count)) {
      for (size_t i = 0; i < srcfiles.count; ++i) {
        if (nob_sv_end_with(nob_sv_from_cstr(srcfiles.items[i]), ".c")) {
          nob_cc_inputs(&cmd, srcfiles.items[i]);
        }
      }
      if (!nob_cmd_run_sync(cmd)) {
        nob_log(NOB_ERROR, "Unable to build deck_ops_plugin");
        nob_cmd_free(cmd);
        ret = 1;
        goto cleanup;
      }
      nob_cmd_free(cmd);
    }
  }
cleanup:
  nob_da_free(srcfiles);
  return ret;
}
