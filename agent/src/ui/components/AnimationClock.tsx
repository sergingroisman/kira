import React, { useState, useEffect, useCallback, useContext, createContext } from "react";

interface AnimationClockValue {
  paused: boolean;
}

const AnimationClockContext = createContext<AnimationClockValue>({ paused: false });

export function useAnimationClock(): AnimationClockValue {
  return useContext(AnimationClockContext);
}

export interface AnimationClockProviderProps {
  children: React.ReactNode;
}

export const AnimationClockProvider: React.FC<AnimationClockProviderProps> = ({
  children,
}) => {
  const [paused, setPaused] = useState(false);

  const handlePause = useCallback(() => setPaused(true), []);
  const handleResume = useCallback(() => setPaused(false), []);

  useEffect(() => {
    process.stdin.on("pause", handlePause);
    process.stdin.on("resume", handleResume);

    return () => {
      process.stdin.off("pause", handlePause);
      process.stdin.off("resume", handleResume);
    };
  }, [handlePause, handleResume]);

  useEffect(() => {
    const handleSig = () => setPaused((p) => !p);
    process.on("SIGTSTP", handleSig);
    process.on("SIGCONT", handleSig);
    return () => {
      process.off("SIGTSTP", handleSig);
      process.off("SIGCONT", handleSig);
    };
  }, []);

  return (
    <AnimationClockContext.Provider value={{ paused }}>
      {children}
    </AnimationClockContext.Provider>
  );
};

export default AnimationClockProvider;
