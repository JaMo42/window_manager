#!/usr/bin/env python3
from math import sqrt
from pyray import *
from pyray import Rectangle as PyrayRect
from dataclasses import dataclass
from typing import Self, Optional, Union
from copy import copy
from pprint import pprint
from abc import ABC, abstractmethod
from random import randrange

def print_once(*x, **kwargs):
    if print_once.first:
        if len(x) == 1:
            pprint(*x, **kwargs)
        else:
            print(*x, **kwargs)
print_once.first = True

@dataclass(unsafe_hash=True)
class Rectangle:
    x: int = 0
    y: int = 0
    width: int = 1
    height: int = 1

    def __iter__(self):
        yield self.x
        yield self.y
        yield self.width
        yield self.height

    def contains(self, point: tuple[int, int]) -> bool:
        x, y = point
        return ((x >= self.x)
            and (x < (self.x + self.width))
            and (y >= self.y)
            and (y < (self.y + self.height)))

    def corners(self):
        yield (self.x, self.y)
        yield (self.x, self.height)
        yield (self.width, self.height)
        yield (self.width, self.y)

    def overlaps(self, other: Self) -> bool:
        for corner in self.corners():
            if other.contains(corner):
                return True
        return False

    def area(self) -> int:
        return self.width * self.height

    def center(self) -> tuple[int, int]:
        return (
            self.x + self.width // 2,
            self.y + self.height // 2,
        )


def center_distance(a: Rectangle, b: Rectangle) -> float:
    ax, ay = a.center()
    bx, by = b.center()
    return sqrt((ax - bx)**2 + (ay - by)**2)


def move_toward(inner: Rectangle, outer: Rectangle, point: tuple[int, int]):
    min_x = outer.x
    max_x = outer.x + outer.width - inner.width
    min_y = outer.y
    max_y = outer.y + outer.height - inner.height
    x = point[0] - inner.width // 2
    y = point[1] - inner.height // 2
    return (
        min(max(x, min_x), max_x),
        min(max(y, min_y), max_y),
    )


def clamp_rect(inner: Rectangle, outer: Rectangle) -> Rectangle:
    min_x = outer.x
    max_x = outer.x + outer.width - inner.width
    min_y = outer.y
    max_y = outer.y + outer.height - inner.height
    inner.x = min(max(inner.x, min_x), max_x)
    inner.y = min(max(inner.y, min_y), max_y)
    return inner


class RectangleMap:
    @dataclass
    class Cell:
        x: int
        y: int
        is_window: bool

    grid: list[list[Cell]]
    width: int
    height: int

    def __init__(self, xs: list[int], ys: list[int], windows: list[Rectangle]):
        self.grid = list()
        for i, y in enumerate(ys):
            row = list()
            for j, x in enumerate(xs):
                is_window = False
                for w in windows:
                    if w.contains((x, y)):
                        is_window = True
                        break
                row.append(self.Cell(x, y, is_window))
            self.grid.append(row)
        self.width = len(xs) - 1
        self.height = len(ys) - 1

    def show(self):
        if not RectangleMap._show_once:
            return
        for row in self.grid[:-1]:
            for cell in row[:-1]:
                print(['#', ' '][cell.is_window], end="")
            print()
        RectangleMap._show_once = False

    def get_rect(self, x: int, y: int, width: int, height: int) -> Rectangle:
        r = Rectangle()
        r.x = self.grid[y][x].x
        r.y = self.grid[y][x].y
        xx = self.grid[y + height][x + width].x
        yy = self.grid[y + height][x + width].y
        r.width = xx - r.x
        r.height = yy - r.y
        return r

    def check(self, x: int, y: int, width: int, height: int) -> False:
        if x + width > self.width or y + height > self.height:
            return False
        for X in range(x, x + width):
            for Y in range(y, y + height):
                if self.grid[Y][X].is_window:
                    return False
        return True

    def iter_origins(self):
        for y in range(self.height):
            for x in range(self.width):
                if not self.grid[y][x].is_window:
                    yield x, y
RectangleMap._show_once = True


class RectangleBuilder(ABC):
    @abstractmethod
    def swap_direction(self): ...

    @abstractmethod
    def try_grow(self) -> bool: ...

    @abstractmethod
    def get(self) -> Rectangle: ...


class AlternatingRectangleBuilder(RectangleBuilder):
    rmap: RectangleMap
    origin: tuple[int, int]
    width: int
    height: int
    down: bool

    def __init__(self, rmap: RectangleMap, origin: tuple[int, int], start_down: bool):
        self.rmap = rmap
        self.origin = origin
        self.width = 1
        self.height = 1
        self.down = start_down

    def swap_direction(self):
        self.down = not self.down

    def try_grow(self) -> bool:
        if self.down:
            self.height += 1
        else:
            self.width += 1
        if not self.rmap.check(*self.origin, self.width, self.height):
            if self.down:
                self.height -= 1
            else:
                self.width -= 1
            return False
        return True

    def get(self) -> Rectangle:
        return self.rmap.get_rect(*self.origin, self.width, self.height)


class AspectRatioedRectangleBuilder(AlternatingRectangleBuilder):
    target: float

    def __init__(self, rmap: RectangleMap, origin: tuple[int, int], start_down: bool, aspect_ratio: float):
        super().__init__(rmap, origin, start_down)
        self.target = aspect_ratio

    def swap_direction(self):
        down_r = self.width / (self.height + 1)
        right_r = (self.width + 1) / self.height
        self.down = abs(down_r - self.target) < abs(right_r - self.target)


def find_open_spaces(
    screen: Rectangle,
    windows: list[Rectangle],
    aspect_ratio: float,
) -> list[Rectangle]:
    # Get edges
    xs = {screen.x}
    ys = {screen.y}
    for w in windows:
        xs.add(w.x)
        xs.add(w.x + w.width)
        ys.add(w.y)
        ys.add(w.y + w.height)
    xs.add(screen.x + screen.width)
    ys.add(screen.y + screen.height)
    xs = sorted(xs)
    ys = sorted(ys)

    for x in xs:
        draw_line(x, 0, x, screen.height, RED)
        if x != xs[-1]:
            draw_text(f"{x}", x, 0, 32, WHITE)
    for y in ys:
        draw_line(0, y, screen.width, y, RED)
        if y != ys[-1]:
            draw_text(f"{y}", 0, y, 32, WHITE)

    m = RectangleMap(xs, ys, windows)
    m.show()
    spaces = set()

    def check(builder: RectangleBuilder, alternate: bool) -> Optional[Rectangle]:
        if alternate:
            while builder.try_grow():
                builder.swap_direction()
            builder.swap_direction()
            while builder.try_grow():
                ...
            return builder.get()
        else:
            while builder.try_grow():
                ...
            builder.swap_direction()
            while builder.try_grow():
                ...
            return builder.get()

    for origin in m.iter_origins():
        #for (start_down, alternate) in ((False, False), (False, True), (True, False), (True, True)):
        #    builder = AlternatingRectangleBuilder(m, origin, start_down)
        #    if (r := check(builder, alternate)) is not None:
        #        spaces.add(r)
        builder = AspectRatioedRectangleBuilder(m, origin, True, aspect_ratio)
        builder.swap_direction()
        if (r := check(builder, True)) is not None:
            spaces.add(r)

    return list(spaces)


def center_distance(a: Union[Rectangle, tuple[int, int]], b: Union[Rectangle, tuple[int, int]]) -> float:
    if isinstance(a, Rectangle):
        ax, ay = a.center()
    else:
        ax, ay = a
    if isinstance(b, Rectangle):
        bx, by = b.center()
    else:
        bx, by = b
    return sqrt((ax - bx)**2 + (ay - by)**2)


@dataclass
class SpaceInfo:
    space: Rectangle
    rect: Rectangle
    width: int
    height: int

    def __init__(self, space: Rectangle, rect: Rectangle, screen: Rectangle):
        self.space = space
        self.width = space.width
        self.height = space.height
        x, y = move_toward(rect, space, screen.center())
        rect = Rectangle(x, y, rect.width, rect.height)
        if rect.width > space.width:
            rect.x = space.x + (space.width - rect.width) // 2
        if rect.height > space.height:
            rect.y = space.y + (space.height - rect.height) // 2
        rect = clamp_rect(rect, screen)
        self.rect = rect

    def center(self) -> tuple[int, int]:
        return self.rect.center()

    def area(self) -> int:
        return self.space.area()

    def position(self) -> tuple[int, int]:
        return self.rect.x, self.rect.y


def find_position(rect: Rectangle, screen: Rectangle, windows: list[Rectangle]) -> tuple[int, int]:
    get_default = lambda: ((screen.width - rect.width) // 2, (screen.height - rect.height) // 2)
    if len(windows) > 9 or rect.width >= screen.width * 0.8 or rect.height >= screen.height  * 0.8:
        # Don't even try for large rectangles or if we already have a lot of windows.
        # 9 was randomly chosen.
        return get_default()

    open_spaces = [SpaceInfo(s, rect, screen) for s in find_open_spaces(screen, windows, rect.width / rect.height)]
    if len(open_spaces) == 0:
        return get_default()
    open_spaces.sort(key=lambda r: center_distance(r.center(), screen), reverse=False)

    idx = None
    for i, space in enumerate(open_spaces):
        if space.width >= rect.width and space.height >= rect.height:
            idx = i
            break
    if idx is None:
        idx = max(range(len(open_spaces)), key=lambda i: open_spaces[i].area())

    space = open_spaces[idx]
    #print_once(space.space)
    draw_rectangle(*space.space, color_alpha(GREEN, 0.3))
    return space.position()


def main():
    SCREEN = Rectangle(0, 0, 1600, 900)
    init_window(SCREEN.width, SCREEN.height, "Rect");
    set_target_fps(10)

    windows = [
        #Rectangle(10, 10, 160*4, 90*4),
        #Rectangle(860, 240, 700, 500),
        #Rectangle(760, 140, 300, 300),
        #Rectangle(0, 900-100, 100, 100),
        #Rectangle(100, 900-100-100, 100, 100),
        Rectangle(x=634, y=672, width=534, height=124),
        Rectangle(x=557, y=530, width=332, height=288),
        Rectangle(x=71, y=90, width=136, height=323),
        Rectangle(x=359, y=387, width=241, height=442),
    ]

    new_window = Rectangle(0, 0, 600, 400)

    MAX_SIZE_PERCENT = 0.5
    while not window_should_close():
        if is_key_released(KeyboardKey.KEY_R):
            print("========")
            for w in windows:
                w.width = randrange(100, int(SCREEN.width * MAX_SIZE_PERCENT))
                w.height = randrange(100, int(SCREEN.height * MAX_SIZE_PERCENT))
                w.x = randrange(0, SCREEN.width - w.width)
                w.y = randrange(0, SCREEN.height - w.height)
                print(f"{w}")
        begin_drawing()
        clear_background(BLACK)
        for w in windows:
            draw_rectangle(*w, SKYBLUE)
            draw_rectangle_lines_ex(PyrayRect(*w), 3.0, WHITE)
        p = find_position(new_window, SCREEN, windows)
        new_window.x, new_window.y = p
        draw_rectangle(*new_window, color_alpha(YELLOW, 0.5))
        end_drawing()
        print_once.first = False

    close_window()

if __name__ == "__main__":
    main()
