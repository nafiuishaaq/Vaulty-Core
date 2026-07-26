'use client'

import { useMemo } from 'react'
import { useStreak } from '@/hooks/useStreak'

const WEEKDAY_LABELS = ['Mon', '', 'Wed', '', 'Fri', '', '']
const MONTH_LABELS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec']

export default function StreakCalendar() {
  const streak = useStreak()

  const { weeks, monthMarkers } = useMemo(() => {
    const cells = streak.calendar.map((day) => ({
      date: day.date,
      deposited: day.deposited,
      dayOfWeek: day.date.getDay(),
      weekIndex: Math.floor((streak.calendar.length - 1 - (streak.calendar.length - 1 - day.date.getDay()) / 7)),
    }))

    const grouped: { date: Date; deposited: boolean }[][] = []
    let currentWeek: { date: Date; deposited: boolean }[] = []

    const firstDayOfWeek = streak.calendar[0]?.date.getDay() ?? 0
    for (let i = 0; i < firstDayOfWeek; i++) {
      currentWeek.push({ date: new Date(0), deposited: false })
    }

    for (const day of streak.calendar) {
      currentWeek.push({ date: day.date, deposited: day.deposited })
      if (currentWeek.length === 7) {
        grouped.push(currentWeek)
        currentWeek = []
      }
    }
    if (currentWeek.length > 0) {
      while (currentWeek.length < 7) {
        currentWeek.push({ date: new Date(0), deposited: false })
      }
      grouped.push(currentWeek)
    }

    const markers: { label: string; weekIndex: number }[] = []
    let lastMonth = -1
    grouped.forEach((week, idx) => {
      const firstRealDay = week.find((d) => d.date.getTime() !== 0)
      if (firstRealDay) {
        const month = firstRealDay.date.getMonth()
        if (month !== lastMonth) {
          markers.push({ label: MONTH_LABELS[month], weekIndex: idx })
          lastMonth = month
        }
      }
    })

    return { weeks: grouped, monthMarkers: markers }
  }, [streak.calendar])

  if (streak.calendar.length === 0) {
    return (
      <div className="rounded-lg border border-slate-200 bg-white p-6 shadow-sm">
        <h2 className="mb-4 text-2xl font-bold text-slate-900">Savings Calendar</h2>
        <p className="text-sm text-slate-400">No deposit data yet.</p>
      </div>
    )
  }

  return (
    <div className="rounded-lg border border-slate-200 bg-white p-6 shadow-sm">
      <h2 className="mb-4 text-2xl font-bold text-slate-900">Savings Calendar</h2>

      <div className="overflow-x-auto" role="region" aria-label="Deposit activity calendar">
        <div className="flex gap-1">
          <div className="flex flex-col gap-1 pr-2">
            {WEEKDAY_LABELS.map((label, i) => (
              <div key={i} className="flex h-3 w-8 items-center justify-end text-[10px] text-slate-400">
                {label}
              </div>
            ))}
          </div>

          <div className="flex gap-1">
            <div className="flex flex-col gap-1">
              {monthMarkers.map((m) => (
                <div key={m.weekIndex} className="h-3 text-[10px] text-slate-500">
                  {weeks[m.weekIndex] === weeks[monthMarkers[0]?.weekIndex] ? '' : ''}
                </div>
              ))}
            </div>

            <div className="flex gap-1">
              {weeks.map((week, wi) => (
                <div key={wi} className="flex flex-col gap-1">
                  {week.map((day, di) => {
                    if (day.date.getTime() === 0) {
                      return <div key={di} className="h-3 w-3" />
                    }
                    return (
                      <div
                        key={di}
                        className="h-3 w-3 rounded-sm"
                        style={{
                          backgroundColor: day.deposited ? '#2563eb' : '#e2e8f0',
                        }}
                        title={
                          day.deposited
                            ? `Deposit on ${day.date.toLocaleDateString(undefined, { month: 'short', day: 'numeric', year: 'numeric' })}`
                            : `No deposit on ${day.date.toLocaleDateString(undefined, { month: 'short', day: 'numeric', year: 'numeric' })}`
                        }
                        role="img"
                        aria-label={
                          day.deposited
                            ? `Deposit made on ${day.date.toLocaleDateString(undefined, { month: 'short', day: 'numeric', year: 'numeric' })}`
                            : `No deposit on ${day.date.toLocaleDateString(undefined, { month: 'short', day: 'numeric', year: 'numeric' })}`
                        }
                      />
                    )
                  })}
                </div>
              ))}
            </div>
          </div>
        </div>

        <div className="mt-3 flex items-center gap-2 text-xs text-slate-500">
          <span>Less</span>
          <div className="h-3 w-3 rounded-sm" style={{ backgroundColor: '#e2e8f0' }} />
          <div className="h-3 w-3 rounded-sm" style={{ backgroundColor: '#2563eb' }} />
          <span>More</span>
        </div>
      </div>
    </div>
  )
}
