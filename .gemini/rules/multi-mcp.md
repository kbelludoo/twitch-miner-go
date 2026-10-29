@GEMINI:MCP:3.0.0%mode=solo%runtime=mbvm

^mcp_rules {
  solo_execution_only!
  tool_schemas_mbvm_1line!
  prohibit_bloated_json_schemas!
}

^R21:mode=strict_solo!direct_agent_execution_only
