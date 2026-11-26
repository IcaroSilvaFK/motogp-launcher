import { useCallback, useEffect, useRef, useState } from 'react';
import { toCommunitySignInError } from '../domain';
import type { CommunityErrorCode, CommunityProfile, CommunityStatus } from '../domain';
import type { CommunityAccount } from './ports/CommunityAccount';

export type CommunityAccountState = 'signedOut' | 'signingIn' | 'signedIn' | 'offline';

export interface CommunityAccountHandle {
  /** False without a port (tests, builds without the community): render no account row at all. */
  available: boolean;
  /** False until the first status answer arrived; the row can hold its place meanwhile. */
  loaded: boolean;
  state: CommunityAccountState;
  /** The member while `signedIn`; `null` otherwise. */
  profile: CommunityProfile | null;
  /** Why the last sign-in failed; cleared when a new one starts or the user signs out. */
  errorCode: CommunityErrorCode | null;
  signIn(): void;
  signOut(): void;
}

interface Current {
  state: CommunityAccountState;
  profile: CommunityProfile | null;
}

function fromStatus(status: CommunityStatus): Current {
  if (status.state === 'signed_in') return { state: 'signedIn', profile: status.profile };
  return { state: status.state === 'offline' ? 'offline' : 'signedOut', profile: null };
}

/**
 * Loads the community status in an effect (nothing awaits it) and drives sign-in and sign-out.
 * Without a port nothing is requested and the handle stays `available: false`.
 */
export function useCommunityAccount(account: CommunityAccount | undefined): CommunityAccountHandle {
  const [current, setCurrent] = useState<Current>({ state: 'signedOut', profile: null });
  const [loaded, setLoaded] = useState(false);
  const [errorCode, setErrorCode] = useState<CommunityErrorCode | null>(null);
  const mounted = useRef(true);
  const busy = useRef(false);

  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);

  useEffect(() => {
    if (!account) return;
    let active = true;
    const done = (status: CommunityStatus) => {
      // A sign-in or sign-out in flight already knows better than a late status answer.
      if (!active || busy.current) return;
      setCurrent(fromStatus(status));
      setLoaded(true);
    };
    try {
      account.status().then(done, () => done({ state: 'offline' }));
    } catch {
      done({ state: 'offline' });
    }
    return () => {
      active = false;
    };
  }, [account]);

  const signIn = useCallback(() => {
    if (!account || busy.current) return;
    busy.current = true;
    setErrorCode(null);
    setCurrent((previous) => ({ ...previous, state: 'signingIn' }));
    let attempt: Promise<CommunityStatus>;
    try {
      attempt = account.signIn();
    } catch (error) {
      attempt = Promise.reject(error);
    }
    attempt.then(
      (status) => {
        busy.current = false;
        if (!mounted.current) return;
        setLoaded(true);
        setCurrent(fromStatus(status));
      },
      (error: unknown) => {
        busy.current = false;
        if (!mounted.current) return;
        const { code } = toCommunitySignInError(error);
        setLoaded(true);
        setErrorCode(code);
        setCurrent({ state: code === 'offline' ? 'offline' : 'signedOut', profile: null });
      },
    );
  }, [account]);

  const signOut = useCallback(() => {
    if (!account || busy.current) return;
    busy.current = true;
    setErrorCode(null);
    const finish = () => {
      busy.current = false;
      if (mounted.current) setCurrent({ state: 'signedOut', profile: null });
    };
    try {
      account.signOut().then(finish, finish);
    } catch {
      finish();
    }
  }, [account]);

  return {
    available: account !== undefined,
    loaded,
    state: current.state,
    profile: current.profile,
    errorCode,
    signIn,
    signOut,
  };
}
