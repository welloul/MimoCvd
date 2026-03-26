#!/usr/bin/env python3
"""
Trading Signal Generator
Generates trading signals from JSON candle data based on CVD flip and swing detection.
"""

import json
import sys
from typing import Dict, List, Optional, Any
from collections import defaultdict
from dataclasses import dataclass, asdict


@dataclass
class Candle:
    """Represents a single candle with OHLCV and CVD data."""
    close: float
    cvd: float
    high: float
    low: float
    open: float
    poc: Optional[float]
    symbol: str
    timestamp: str
    trade_count: int
    volume: float


@dataclass
class Signal:
    """Represents a trading signal or rejection."""
    symbol: str
    timestamp: str
    signal_type: str  # LONG, SHORT, or NONE
    rejection_reason: Optional[str] = None
    is_new_high: bool = False
    is_new_low: bool = False
    current_cvd: float = 0.0
    previous_cvd: float = 0.0
    midpoint: float = 0.0
    close: float = 0.0
    high: float = 0.0
    low: float = 0.0


class SignalGenerator:
    """Generates trading signals based on candle data and CVD analysis."""
    
    def __init__(self, lookback: int = 20):
        self.lookback = lookback
        # Store candle history per symbol
        self.candle_history: Dict[str, List[Candle]] = defaultdict(list)
        # Track positions (symbol -> has_position)
        self.positions: Dict[str, bool] = {}
    
    def load_candles_from_file(self, filepath: str) -> List[Candle]:
        """Load candles from a JSON lines file."""
        candles = []
        with open(filepath, 'r') as f:
            for line in f:
                line = line.strip()
                if line:
                    data = json.loads(line)
                    candle = Candle(
                        close=data['close'],
                        cvd=data['cvd'],
                        high=data['high'],
                        low=data['low'],
                        open=data['open'],
                        poc=data.get('poc'),
                        symbol=data['symbol'],
                        timestamp=data['timestamp'],
                        trade_count=data.get('trade_count', 0),
                        volume=data.get('volume', 0.0)
                    )
                    candles.append(candle)
        return candles
    
    def check_preconditions(self, symbol: str, candle: Candle) -> Optional[str]:
        """
        Check pre-conditions for signal generation.
        Returns rejection reason if any condition fails, None if all pass.
        """
        # Check if already in position
        if self.positions.get(symbol, False):
            return "Rejected - existing position for symbol"
        
        # Check if POC exists
        if candle.poc is None:
            return "Rejected - no POC for current candle"
        
        # Check sufficient history
        if len(self.candle_history[symbol]) < self.lookback:
            return f"Rejected - insufficient history ({len(self.candle_history[symbol])} < {self.lookback})"
        
        return None
    
    def detect_swing(self, symbol: str, candle: Candle) -> tuple[bool, bool]:
        """
        Detect if current candle is a new swing high or low.
        Returns (is_new_high, is_new_low)
        """
        history = self.candle_history[symbol]
        
        # Get lookback candles (excluding current)
        if len(history) >= self.lookback:
            lookback_candles = history[-self.lookback:]
        else:
            lookback_candles = history[:-1] if len(history) > 1 else []
        
        if not lookback_candles:
            return False, False
        
        # Calculate highest high and lowest low in lookback period
        highest_high = max(c.high for c in lookback_candles)
        lowest_low = min(c.low for c in lookback_candles)
        
        # Check for new swing high/low
        is_new_high = candle.high > highest_high
        is_new_low = candle.low < lowest_low
        
        return is_new_high, is_new_low
    
    def determine_direction(self, candle: Candle, is_new_high: bool, is_new_low: bool) -> tuple[str, float]:
        """
        Determine signal direction based on swing type and price action.
        Returns (direction, midpoint)
        """
        midpoint = (candle.high + candle.low) / 2.0
        
        if is_new_high and candle.close < midpoint:
            return "SHORT", midpoint
        elif is_new_low and candle.close > midpoint:
            return "LONG", midpoint
        else:
            return "NONE", midpoint
    
    def check_cvd_flip(self, symbol: str, candle: Candle, direction: str) -> bool:
        """
        Check if CVD flip confirms the signal direction.
        Returns True if CVD flip matches direction, False otherwise.
        """
        history = self.candle_history[symbol]
        
        if len(history) < 2:
            return False
        
        # Get previous candle's CVD
        previous_candle = history[-2]
        prev_cvd = previous_candle.cvd
        curr_cvd = candle.cvd
        
        if direction == "SHORT":
            # SHORT: Previous CVD > 0 AND Current CVD < 0 (Positive → Negative)
            return prev_cvd > 0 and curr_cvd < 0
        elif direction == "LONG":
            # LONG: Previous CVD < 0 AND Current CVD > 0 (Negative → Positive)
            return prev_cvd < 0 and curr_cvd > 0
        else:
            return False
    
    def generate_signal(self, candle: Candle) -> Signal:
        """
        Generate a trading signal for the given candle.
        """
        symbol = candle.symbol
        
        # Step 1: Check pre-conditions
        rejection_reason = self.check_preconditions(symbol, candle)
        if rejection_reason:
            # Add candle to history before returning
            self.candle_history[symbol].append(candle)
            if len(self.candle_history[symbol]) > self.lookback + 10:
                self.candle_history[symbol].pop(0)
            
            return Signal(
                symbol=symbol,
                timestamp=candle.timestamp,
                signal_type="NONE",
                rejection_reason=rejection_reason,
                close=candle.close,
                high=candle.high,
                low=candle.low
            )
        
        # Step 2: Detect swing
        is_new_high, is_new_low = self.detect_swing(symbol, candle)
        
        if not is_new_high and not is_new_low:
            # Add candle to history before returning
            self.candle_history[symbol].append(candle)
            if len(self.candle_history[symbol]) > self.lookback + 10:
                self.candle_history[symbol].pop(0)
            
            return Signal(
                symbol=symbol,
                timestamp=candle.timestamp,
                signal_type="NONE",
                rejection_reason="Rejected - not a new swing",
                is_new_high=is_new_high,
                is_new_low=is_new_low,
                close=candle.close,
                high=candle.high,
                low=candle.low
            )
        
        # Step 3: Determine direction
        direction, midpoint = self.determine_direction(candle, is_new_high, is_new_low)
        
        if direction == "NONE":
            # Add candle to history before returning
            self.candle_history[symbol].append(candle)
            if len(self.candle_history[symbol]) > self.lookback + 10:
                self.candle_history[symbol].pop(0)
            
            return Signal(
                symbol=symbol,
                timestamp=candle.timestamp,
                signal_type="NONE",
                rejection_reason="Rejected - signal determination returned None",
                is_new_high=is_new_high,
                is_new_low=is_new_low,
                midpoint=midpoint,
                close=candle.close,
                high=candle.high,
                low=candle.low
            )
        
        # Step 4: Check CVD flip
        cvd_flip_confirmed = self.check_cvd_flip(symbol, candle, direction)
        
        # Get previous CVD for output
        prev_cvd = self.candle_history[symbol][-2].cvd if len(self.candle_history[symbol]) >= 2 else 0.0
        
        if not cvd_flip_confirmed:
            # Add candle to history before returning
            self.candle_history[symbol].append(candle)
            if len(self.candle_history[symbol]) > self.lookback + 10:
                self.candle_history[symbol].pop(0)
            
            return Signal(
                symbol=symbol,
                timestamp=candle.timestamp,
                signal_type="NONE",
                rejection_reason="Rejected - no CVD flip",
                is_new_high=is_new_high,
                is_new_low=is_new_low,
                current_cvd=candle.cvd,
                previous_cvd=prev_cvd,
                midpoint=midpoint,
                close=candle.close,
                high=candle.high,
                low=candle.low
            )
        
        # All conditions met - generate signal
        signal = Signal(
            symbol=symbol,
            timestamp=candle.timestamp,
            signal_type=direction,
            rejection_reason=None,
            is_new_high=is_new_high,
            is_new_low=is_new_low,
            current_cvd=candle.cvd,
            previous_cvd=prev_cvd,
            midpoint=midpoint,
            close=candle.close,
            high=candle.high,
            low=candle.low
        )
        
        # Add candle to history
        self.candle_history[symbol].append(candle)
        if len(self.candle_history[symbol]) > self.lookback + 10:
            self.candle_history[symbol].pop(0)
        
        return signal
    
    def process_candles(self, candles: List[Candle]) -> List[Dict[str, Any]]:
        """
        Process all candles and generate signals.
        Returns list of signal dictionaries.
        """
        signals = []
        
        for candle in candles:
            signal = self.generate_signal(candle)
            signals.append(asdict(signal))
        
        return signals


def main():
    """Main entry point for the signal generator."""
    if len(sys.argv) < 2:
        print("Usage: python signal_generator.py <input_file> [lookback_period]")
        print("  input_file: Path to JSON lines file with candle data")
        print("  lookback_period: Number of candles for swing detection (default: 20)")
        sys.exit(1)
    
    input_file = sys.argv[1]
    lookback = int(sys.argv[2]) if len(sys.argv) > 2 else 20
    
    # Create signal generator
    generator = SignalGenerator(lookback=lookback)
    
    # Load candles from file
    candles = generator.load_candles_from_file(input_file)
    
    # Process candles and generate signals
    signals = generator.process_candles(candles)
    
    # Output as JSON
    print(json.dumps(signals, indent=2))


if __name__ == "__main__":
    main()