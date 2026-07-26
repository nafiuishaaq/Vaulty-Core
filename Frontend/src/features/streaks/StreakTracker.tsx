'use client'

import { useStreak } from '@/hooks/useStreak'

export default function StreakTracker() {
  const streak = useStreak()

  return (
    <div className="rounded-lg border border-slate-200 bg-white p-6 shadow-sm">
      <h2 className="mb-4 text-2xl font-bold text-slate-900">Streak Tracker</h2>

      <div className="grid grid-cols-3 gap-4">
        <div className="rounded-md bg-blue-50 p-4 text-center">
          <p className="text-3xl font-bold text-blue-600">{streak.currentStreak}</p>
          <p className="mt-1 text-sm text-slate-600">Current Streak</p>
        </div>

        <div className="rounded-md bg-purple-50 p-4 text-center">
          <p className="text-3xl font-bold text-purple-600">{streak.longestStreak}</p>
          <p className="mt-1 text-sm text-slate-600">Longest Streak</p>
        </div>

        <div className="rounded-md bg-amber-50 p-4 text-center">
          <p className="text-3xl font-bold text-amber-600">{streak.freezesRemaining}</p>
          <p className="mt-1 text-sm text-slate-600">Freezes Available</p>
        </div>
      </div>

      {streak.lastDepositDate && (
        <p className="mt-4 text-sm text-slate-500">
          Last deposit:{' '}
          {streak.lastDepositDate.toLocaleDateString(undefined, {
            year: 'numeric',
            month: 'long',
            day: 'numeric',
          })}
        </p>
      )}

      {streak.currentStreak > 0 && (
        <p className="mt-2 text-sm text-green-600">
          Keep it going! You&apos;re on a {streak.currentStreak}-day streak.
        </p>
      )}

      {streak.currentStreak === 0 && streak.lastDepositDate && (
        <p className="mt-2 text-sm text-slate-500">
          Make a deposit to start a new streak.
        </p>
      )}

      {!streak.lastDepositDate && (
        <p className="mt-2 text-sm text-slate-400">
          No deposits yet. Make your first deposit to start tracking streaks.
        </p>
      )}
    </div>
  )
}
