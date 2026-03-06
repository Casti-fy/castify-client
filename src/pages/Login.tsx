import { useState } from "react";
import type { User } from "../lib/types";
import * as api from "../lib/api";

interface Props {
  onLogin: (user: User) => void;
}

export default function Login({ onLogin }: Props) {
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [isRegister, setIsRegister] = useState(false);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const emailValid = /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email);

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (isRegister && !emailValid) {
      setError("Please enter a valid email address");
      return;
    }
    setLoading(true);
    setError(null);
    try {
      const resp = isRegister
        ? await api.register(email, password)
        : await api.login(email, password);
      onLogin(resp.user);
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="center">
      <form onSubmit={submit} className="login-form">
        <h1>Castify</h1>

        <input
          type="email"
          placeholder="Email"
          value={email}
          onChange={(e) => setEmail(e.target.value)}
          autoFocus
        />

        <input
          type="password"
          placeholder="Password"
          value={password}
          onChange={(e) => setPassword(e.target.value)}
        />

        {error && <p className="error">{error}</p>}

        <button
          type="submit"
          className="btn primary"
          disabled={loading || !email || !password || (isRegister && !emailValid)}
        >
          {loading ? "..." : isRegister ? "Register" : "Login"}
        </button>

        <button
          type="button"
          className="btn link"
          onClick={() => {
            setIsRegister(!isRegister);
            setError(null);
          }}
        >
          {isRegister
            ? "Already have an account? Login"
            : "Need an account? Register"}
        </button>
      </form>
    </div>
  );
}
