@CLAUDE:3.6.0%cli=claude%lang=go+rust%runtime=mbvm%mode=solo%engine=mbvm-db

^claude {
  follow_AGENTS_mbvm!
  global="~/.claude/AGENTS.mbvm"!
  project="./AGENTS.mbvm"!
}

^ops {
  solo_agent_execution!
  dual_languages[go1.27,rust]!
  single_ingestion_law!
  mbvm_db_first[mbvm-db slice <db> <symbol/term> <depth>]!
  never_read_raw_catalogs_or_protobufs!
  codebase_mbvm_root!
  context_before_dump!
  go_checks[go_vet,go_test_-race_-count=1_-v]!
  rust_checks[cargo_clippy_--_-D_warnings,cargo_test_--all-features,no_unwrap]!
  root_cause_first!
  targeted_minimal_diffs!
  modern_stdlib[slog,slices,unique,iter]!
  err_wrap_w_only!no_v_in_errors!
  consumer_interfaces!
  goroutine_leak_prevention[ctx_done_select]!
  rust_errors[thiserror_or_anyhow]!
  tokio_spawn_blocking_for_sync_io!
  safety_comments_on_unsafe!
  no_secrets!maskSecret()!
  max_loc_400!
  skills_mandatory!
  hook[git_global_pre_commit]!
}

^R0:mbvm_canonical!
^R1:solo_agent_only!
^R8:dual_primary[go1.27,rust]!
^R10:compiler_as_truth!
^R12:root_cause_first!
^R14:mbvm_db_slicing!
^R15:mbvm_first!
^R21:mode=strict_solo!
^R22:context_rich=per_project!engine=mbvm-db!subgraph_slice!codebase_mbvm!full_project_ast_map
