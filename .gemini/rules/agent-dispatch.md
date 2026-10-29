@GEMINI:DISPATCH:3.0.0%mode=solo

^dispatch {
  solo_direct_resolution!
  single_agent_autonomous!
  return_structured:{status,summary,files_changed}
}

^R21:mode=strict_solo!direct_agent_execution_only
