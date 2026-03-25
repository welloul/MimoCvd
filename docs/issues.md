# CVDTrader Known Issues and Tracking

## Open Issues

### High Priority
1. **WebSocket Reconnection Under Extreme Network Conditions**
   - Description: Exponential backoff may not be optimal for all network partition scenarios
   - Impact: Medium - could delay recovery during prolonged outages
   - Status: Open
   - Assigned: None

### Medium Priority
4. **Configuration Hot Reloading**
   - Description: Requires bot restart to pick up configuration changes
   - Impact: Low-Medium - operational inconvenience
   - Status: Open
   - Assigned: None

5. **Volume Profile Optimization**
   - Description: Rebuilds volume profile per candle instead of maintaining rolling window
   - Impact: Low - minor performance impact
   - Status: Open
   - Assigned: None

### Low Priority
6. **Unused Imports**
   - Description: Some modules have unused imports (e.g., cvd_poc.rs had Side, Signal, DateTime)
   - Impact: Low - cosmetic issue, doesn't affect functionality
   - Status: Closed (cleaned up in latest commit)
   - Assigned: None

## Closed Issues

### Resolved in Session: March 25, 2026
1. **Signal Generator History Integration** ✅
   - Description: SignalEvaluator uses empty history vector instead of pulling from GlobalState
   - Impact: High - affects signal accuracy until lookback period is satisfied
   - Resolution: Updated process_candle to retrieve candle history from GlobalState and pass to evaluate_signal
   - Closed: March 25, 2026

2. **VWAP Integration in Strategy** ✅
   - Description: Strategy signals don't use actual VWAP data (marked as TODO)
   - Impact: Medium - reduces signal quality and accuracy
   - Resolution: VWAP module removed from codebase - not needed for current strategy implementation
   - Closed: March 25, 2026

### Resolved in Session: March 24, 2026
3. **Risk Manager Account Balance** ✅
   - Description: Uses hardcoded 10000.0 balance rather than configurable value
   - Impact: Medium - inaccurate leverage calculations
   - Resolution: Added account_balance to RiskConfig, hybrid approach (config for dryrun, API for live)
   - Closed: March 24, 2026

2. **Error Type Specificity** ✅
   - Description: Many functions return generic anyhow::Error instead of specific error types
   - Impact: Low - makes error handling less precise
   - Resolution: Created ExecutionError enum with specific error types (Network, Validation, Exchange, Timeout, RateLimited, etc.)
   - Closed: March 24, 2026

3. **Channel Buffer Sizing** ✅
   - Description: Fixed channel sizes (1000 trades, 100 candles) may not suit all workloads
   - Impact: Low - could cause backpressure or underutilization
   - Resolution: Added trade_buffer_size and candle_buffer_size to BotConfig
   - Closed: March 24, 2026

4. **TTL Checker Granularity** ✅
   - Description: Checks for expired orders every 10 seconds (could be more frequent)
   - Impact: Low - orders may stay open up to 10 seconds past TTL
   - Resolution: Added ttl_check_interval_secs to ExecutionConfig
   - Closed: March 24, 2026

5. **Hardcoded Tick Sizes** ✅
   - Description: Some modules use hardcoded tick sizes rather than config-derived values
   - Impact: Low - affects precision for different instruments
   - Resolution: Fetch tick sizes from exchange metadata on startup, pass to strategy
   - Closed: March 24, 2026

6. **Unused Imports** ✅
   - Description: cvd_poc.rs had unused imports (Side, Signal, DateTime)
   - Impact: Low - cosmetic issue
   - Resolution: Removed unused imports, kept only what's needed
   - Closed: March 24, 2026

7. **Trade History Persistence** ✅
   - Description: No mechanism to record completed trades for analysis
   - Impact: Medium - unable to track performance over time
   - Resolution: Added TradeHistory module with SQLite database (trades.db)
   - Closed: March 24, 2026

8. **Health Check Endpoint** ✅
   - Description: No way to monitor bot status externally
   - Impact: Low - operational visibility
   - Resolution: Added HealthServer with /health endpoint (configurable port)
   - Closed: March 24, 2026

## Bug Reporting Template
When reporting issues, please include:
1. **Version**: Git commit hash or version tag
2. **Environment**: OS, Rust version, cargo version
3. **Configuration**: Relevant config.toml sections (remove secrets)
4. **Steps to Reproduce**: Detailed steps to reproduce the issue
5. **Expected Behavior**: What should happen
6. **Actual Behavior**: What actually happens
7. **Logs**: Relevant log output (with secrets removed)
8. **Minimal Reproduction**: If possible, a minimal code snippet that demonstrates the issue

## Feature Request Template
For feature requests, please include:
1. **Problem Statement**: Clear description of the problem
2. **Proposed Solution**: How you'd like to see it solved
3. **Alternatives Considered**: Other approaches you've thought about
4. **Impact**: Who would benefit and how much
5. **Implementation Notes**: Any thoughts on how to implement it

## Contribution Guidelines
1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Make your changes
4. Ensure all tests pass (`cargo test --release`)
5. Update documentation as needed
6. Submit a pull request with a clear description of changes

## Security Reporting
For security vulnerabilities, please contact [security@example.com] directly rather than opening a public issue.