@GEMINI:SECURITY:3.0.0

^security {
  no_secrets_exposed!
  maskSecrets()!
  sanitized_payloads!
  no_eval!
  no_unsafe!
  least_privilege!
}

^R21:mode=strict_solo!direct_agent_execution_only
