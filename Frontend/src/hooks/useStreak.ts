import { useMemo } from 'react'
import { useAppStore } from '@/stores'
import { Streak, StreakDay } from '@/types'

const MS_PER_DAY = 86400000

function getLocalDate(date: Date): string {
  const y = date.getFullYear()
  const m = String(date.getMonth() + 1).padStart(2, '0')
  const d = String(date.getDate()).padStart(2, '0')
  return `${y}-${m}-${d}`
}

function parseLocalDate(str: string): Date {
  const [y, m, d] = str.split('-').map(Number)
  return new Date(y, m - 1, d)
}

export function useStreak(): Streak {
  const vaults = useAppStore((s) => s.vaults)

  return useMemo(() => {
    const depositDates = new Set<string>()

    for (const vault of vaults) {
      for (const deposit of vault.deposits) {
        const confirmed = deposit.transactionHash && deposit.transactionHash.length > 0
        if (confirmed) {
          depositDates.add(getLocalDate(new Date(deposit.timestamp)))
        }
      }
    }

    const sortedDates = Array.from(depositDates).sort()
    if (sortedDates.length === 0) {
      return { currentStreak: 0, longestStreak: 0, freezesRemaining: 0, lastDepositDate: null, calendar: [] }
    }

    const today = getLocalDate(new Date())
    const todayDate = parseLocalDate(today)

    let currentStreak = 0
    let longestStreak = 0
    let streakCount = 0
    const lastDeposit = sortedDates[sortedDates.length - 1]
    const lastDepositDate = parseLocalDate(lastDeposit)

    for (let i = 0; i < sortedDates.length; i++) {
      if (i === 0) {
        streakCount = 1
      } else {
        const prev = parseLocalDate(sortedDates[i - 1])
        const curr = parseLocalDate(sortedDates[i])
        const diffMs = curr.getTime() - prev.getTime()
        const diffDays = Math.round(diffMs / MS_PER_DAY)
        if (diffDays === 1) {
          streakCount++
        } else {
          streakCount = 1
        }
      }
      longestStreak = Math.max(longestStreak, streakCount)
    }

    const diffFromTodayMs = todayDate.getTime() - lastDepositDate.getTime()
    const diffFromTodayDays = Math.round(diffFromTodayMs / MS_PER_DAY)

    if (diffFromTodayDays === 0 || diffFromTodayDays === 1) {
      currentStreak = streakCount
    } else {
      currentStreak = 0
    }

    const calendar: StreakDay[] = []
    const lookback = 90
    for (let i = lookback - 1; i >= 0; i--) {
      const d = new Date(todayDate)
      d.setDate(d.getDate() - i)
      const dateStr = getLocalDate(d)
      calendar.push({
        date: d,
        deposited: depositDates.has(dateStr),
      })
    }

    const freezesRemaining = 2

    return {
      currentStreak,
      longestStreak,
      freezesRemaining,
      lastDepositDate,
      calendar,
    }
  }, [vaults])
}
