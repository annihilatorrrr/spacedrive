import { ExplorerLayoutMode, useExplorerStore } from '@sd/client';
import { ExplorerContext, ExplorerItem, FilePath } from '@sd/core';
import { useVirtualizer } from '@tanstack/react-virtual';
import React, { memo, useCallback, useLayoutEffect, useRef, useState } from 'react';
import { useSearchParams } from 'react-router-dom';
import { useKey, useWindowSize } from 'rooks';

import FileItem from './FileItem';
import FileRow from './FileRow';
import { isPath } from './utils';

const TOP_BAR_HEIGHT = 50;

interface Props {
	context: ExplorerContext;
	data: ExplorerItem[];
}

export const FileList: React.FC<Props> = (props) => {
	// const size = useWindowSize();
	const [goingUp, setGoingUp] = useState(false);

	const { selectedRowIndex, layoutMode } = useExplorerStore((state) => ({
		selectedRowIndex: state.selectedRowIndex,
		layoutMode: state.layoutMode
	}));

	const set = useExplorerStore.getState().set;

	useKey('ArrowUp', (e) => {
		e.preventDefault();
		setGoingUp(true);
		if (selectedRowIndex !== -1 && selectedRowIndex !== 0)
			set({ selectedRowIndex: selectedRowIndex - 1 });
	});

	useKey('ArrowDown', (e) => {
		e.preventDefault();
		setGoingUp(false);
		if (selectedRowIndex !== -1 && selectedRowIndex !== (props.data.length ?? 1) - 1)
			set({ selectedRowIndex: selectedRowIndex + 1 });
	});

	// const Header = () => (
	// 	<div>
	// 		{props.context.name && (
	// 			<h1 className="pt-20 pl-4 text-xl font-bold ">{props.context.name}</h1>
	// 		)}
	// 		<div className="table-head">
	// 			<div className="flex flex-row p-2 table-head-row">
	// 				{columns.map((col) => (
	// 					<div
	// 						key={col.key}
	// 						className="relative flex flex-row items-center pl-2 table-head-cell group"
	// 						style={{ width: col.width }}
	// 					>
	// 						<EllipsisHorizontalIcon className="absolute hidden w-5 h-5 -ml-5 cursor-move group-hover:block drag-handle opacity-10" />
	// 						<span className="text-sm font-medium text-gray-500">{col.column}</span>
	// 					</div>
	// 				))}
	// 			</div>
	// 		</div>
	// 	</div>
	// );

	return (
		<div style={{ marginTop: -TOP_BAR_HEIGHT }} className="w-full pl-2 cursor-default">
			<Virtualizer items={props.data} />
		</div>
	);
};

function Virtualizer({ items }: { items: ExplorerItem[] }) {
	const parentRef = useRef<HTMLDivElement>(null);
	const innerRef = useRef<HTMLDivElement>(null);

	const { gridItemSize, layoutMode, listItemSize, selectedRowIndex } = useExplorerStore(
		(state) => ({
			selectedRowIndex: state.selectedRowIndex,
			gridItemSize: state.gridItemSize,
			layoutMode: state.layoutMode,
			listItemSize: state.listItemSize
		})
	);

	const [width, setWidth] = useState(0);

	useLayoutEffect(() => {
		setWidth(innerRef.current?.offsetWidth || 0);
	}, []);

	const amountOfColumns = Math.floor(width / gridItemSize) || 8,
		amountOfRows = layoutMode === 'grid' ? Math.ceil(items.length / amountOfColumns) : items.length,
		itemSize = layoutMode === 'grid' ? gridItemSize + 25 : listItemSize;

	const rowVirtualizer = useVirtualizer({
		count: amountOfRows,
		getScrollElement: () => parentRef.current,
		overscan: 500,
		estimateSize: () => itemSize,
		measureElement: (index) => itemSize
	});

	return (
		<div ref={parentRef} className="h-screen custom-scroll explorer-scroll">
			<div
				ref={innerRef}
				style={{
					height: `${rowVirtualizer.getTotalSize()}px`,
					marginTop: `${TOP_BAR_HEIGHT}px`
				}}
				className="relative w-full"
			>
				{rowVirtualizer.getVirtualItems().map((virtualRow) => (
					<div
						style={{
							height: `${virtualRow.size}px`,
							transform: `translateY(${virtualRow.start}px)`
						}}
						className="absolute top-0 left-0 flex w-full"
						key={virtualRow.key}
					>
						{layoutMode === 'list' ? (
							<WrappedItem
								kind="list"
								isSelected={selectedRowIndex === virtualRow.index}
								index={virtualRow.index}
								item={items[virtualRow.index]}
							/>
						) : (
							[...Array(amountOfColumns)].map((_, i) => {
								const index = virtualRow.index * amountOfColumns + i;
								const item = items[index];
								return (
									<div key={index} className="w-32 h-32">
										<div className="flex">
											{item && (
												<WrappedItem
													kind="grid"
													isSelected={selectedRowIndex === index}
													index={index}
													item={item}
												/>
											)}
										</div>
									</div>
								);
							})
						)}
					</div>
				))}
			</div>
		</div>
	);
}

interface WrappedItemProps {
	item: ExplorerItem;
	index: number;
	isSelected: boolean;
	kind: ExplorerLayoutMode;
}

// Wrap either list item or grid item with click logic as it is the same for both
const WrappedItem: React.FC<WrappedItemProps> = memo(({ item, index, isSelected, kind }) => {
	const [_, setSearchParams] = useSearchParams();

	const onDoubleClick = useCallback(() => {
		if (isPath(item) && item.is_dir) setSearchParams({ path: item.materialized_path });
	}, [item, setSearchParams]);

	const onClick = useCallback(() => {
		useExplorerStore.getState().set({ selectedRowIndex: isSelected ? -1 : index });
	}, [isSelected, index]);

	if (kind === 'list') {
		return (
			<FileRow
				selected={isSelected}
				data={item}
				onClick={onClick}
				onDoubleClick={onDoubleClick}
				index={index}
			/>
		);
	}

	return (
		<FileItem
			onDoubleClick={onDoubleClick}
			index={index}
			data={item}
			selected={isSelected}
			onClick={onClick}
			// size={100}
		/>
	);
});
