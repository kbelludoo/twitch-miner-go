@AGENTS:CONTEXT:3.2.0%runtime=mbvm%lang=go%engine=mbvm-db

^context {
  mbvm_db_slicing_first[mbvm-db slice . <symbol> <depth>]!
  codebase_mbvm_root!
  compile_via_go_tools[go-tools mbvm .]!
  inspect_via_mbvm_db[mbvm-db inspect .]!
  total_coverage_100_percent!
  zero_obsolescence!
  no_hallucination!
  recompile_mbvm_on_change!
}

^R21:mode=strict_solo!direct_agent_execution_only
