@GEMINI:BACKUP:3.0.0

^backup {
  git_on_change!
  gdrive_daily!
}

^R21:mode=strict_solo!direct_agent_execution_only
