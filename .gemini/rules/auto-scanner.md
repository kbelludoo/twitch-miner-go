@GEMINI:SCANNER:3.0.0%lang=go%runtime=mbvm

^scanner {
  scan_before_commit!
  script[.\scan.ps1]!
  hook[auto-scan.ps1 -OnCommit]!
  generate_codebase_mbvm[go-tools mbvm .]!
  no_verify_prohibited!
  bug_hunt!
  security_audit!
}

^R21:mode=strict_solo!direct_agent_execution_only
