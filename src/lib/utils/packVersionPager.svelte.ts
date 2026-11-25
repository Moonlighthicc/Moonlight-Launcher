export const PACK_VERSION_PAGE_SIZE = 30;

export class PackVersionPager {
	query = $state("");
	limit = $state(PACK_VERSION_PAGE_SIZE);
	showAll = $state(false);

	setQuery(query: string) {
		this.query = query;
		this.limit = PACK_VERSION_PAGE_SIZE;
	}

	get searching() {
		return this.query.trim().length > 0;
	}

	get expanded() {
		return this.searching || this.showAll;
	}

	loadMore() {
		this.limit += PACK_VERSION_PAGE_SIZE;
	}

	reset() {
		this.setQuery("");
		this.showAll = false;
	}
}